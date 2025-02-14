//! A multi-producer, multi-consumer broadcast. Each sent value is seen by all
//! consumers.
//!
//! Create a channel using the [`channel()`] function, which returns a
//! [`Sender`] and [`Receiver`] pair used to send and receive the value,
//! respectively.
//!
//! If the senders send faster than any single receiver can receive, that
//! receiver becomes lagged. A [`RecvError::Lagged`] error is returned with the
//! number of messages dropped. The receiver will start receiving from the
//! oldest message that is still in the channel.
//!
//! # Example
//!
//! ```
//! let (tx, rx) = mpsc::bounded(4);
//! let rx2 = tx.subscribe();
//!
//! spawn(async move {
//! 	assert_eq!(rx.recv().await, "hello");
//! 	assert_eq!(rx.recv().await, "world");
//! })
//! .await;
//!
//! spawn(async move {
//! 	assert_eq!(rx2.recv().await, "hello");
//! 	assert_eq!(rx2.recv().await, "world");
//! })
//! .await;
//!
//! tx.send("hello").await;
//! tx.send("world").await;
//! ```
//!
//! When all [`Sender`]s have been dropped and after the remaining messages in
//! the channel have been received, further calls to [`Receiver::recv`] will
//! return [`RecvError::Closed`]
//!
//! When all [`Receiver`]s have been dropped, further calls to [`Sender::send`]
//! will return a [`SendError`]. A receiver can be subscribed, reopening
//! the channel.

use std::mem::replace;

use super::*;
use crate::sync::Backoff;

struct Slot<T> {
	sequence: AtomicUsize,
	remaining: AtomicUsize,
	data: RwLock<MaybeUninit<T>>
}

struct Channel<T> {
	tail: AtomicUsize,
	slots: Box<[Slot<T>]>,
	tx_count: AtomicUsize,
	rx_count: AtomicUsize,
	rx_waiters: ThreadSafeWaitList
}

#[allow(clippy::missing_panics_doc)]
impl<T> Channel<T> {
	#[allow(clippy::expect_used)]
	fn new(mut size: usize) -> Self {
		assert!(size != 0, "Cannot create a zero sized channel");

		size = size.checked_next_power_of_two().expect("Channel too big");

		let mut slots = Vec::with_capacity(size);

		for i in 0..size {
			slots.push(Slot {
				sequence: AtomicUsize::new(i),
				remaining: AtomicUsize::new(0),
				data: RwLock::new(MaybeUninit::uninit())
			});
		}

		Self {
			tail: AtomicUsize::new(0),
			slots: slots.into_boxed_slice(),
			tx_count: AtomicUsize::new(0),
			rx_count: AtomicUsize::new(0),
			rx_waiters: ThreadSafeWaitList::new()
		}
	}

	#[allow(clippy::arithmetic_side_effects)]
	const fn mask(&self) -> usize {
		self.slots.len() - 1
	}

	fn drop_all(&mut self) {
		for slot in &mut self.slots {
			if replace(slot.remaining.get_mut(), 0) != 0 {
				#[allow(clippy::unwrap_used)]
				let data = slot.data.get_mut().unwrap();

				/* Safety: value is initialized */
				unsafe { data.assume_init_drop() };
			}
		}
	}
}

impl<T> Drop for Channel<T> {
	fn drop(&mut self) {
		struct Guard<'a, T> {
			this: &'a mut Channel<T>
		}

		impl<T> Drop for Guard<'_, T> {
			fn drop(&mut self) {
				self.this.drop_all();
			}
		}

		let guard = Guard { this: self };

		guard.this.drop_all();

		forget(guard);
	}
}

/* Safety: T is send */
unsafe impl<T: Send> Send for Channel<T> {}

/* Safety: T is send */
unsafe impl<T: Send> Sync for Channel<T> {}

/// The error returned from a call to `recv`
#[errors]
pub enum RecvError {
	/// The channel is currently empty. Note that async `recv`s can return this
	/// variant if the current task gets interrupted
	#[display("Channel empty")]
	#[kind = ErrorKind::WouldBlock]
	Empty,

	/// The channel is closed
	#[display("Channel closed")]
	Closed,

	/// The receiver lagged and lost some messages
	#[display("Channel lagged by {}", f0)]
	Lagged(usize)
}

/// The error returned from a call to `send`
#[errors(?Debug + ?Display)]
#[fmt("Channel closed")]
pub struct SendError<T>(pub T);

type RecvResult<T> = result::Result<T, RecvError>;
type SendResult<T> = result::Result<(), SendError<T>>;

/// The receivers for a broadcast channel
pub struct Receiver<T> {
	channel: Arc<Channel<T>>,
	pos: usize
}

#[asynchronous]
impl<T: Clone> Receiver<T> {
	fn new(channel: Arc<Channel<T>>) -> Self {
		channel.rx_count.fetch_add(1, Ordering::SeqCst);

		let tail = channel.tail.load(Ordering::SeqCst);

		Self { channel, pos: tail }
	}

	/// Create a new receiver that will receive messages sent after this call
	#[must_use]
	pub fn resubscribe(&self) -> Self {
		Self::new(self.channel.clone())
	}

	/// Reset the current receiver to receive messages sent after this call
	pub fn synchronize(&mut self) {
		self.pos = self.channel.tail.load(Ordering::SeqCst);
	}

	/// Attempt to receive a value in the channel without suspending
	#[allow(clippy::missing_panics_doc, clippy::comparison_chain)]
	pub fn try_recv(&mut self) -> RecvResult<T> {
		let mask = self.channel.mask();

		/* Safety: masked */
		let slot = unsafe { self.channel.slots.get_unchecked(self.pos & mask) };

		#[allow(clippy::unwrap_used)]
		let data = slot.data.read().unwrap();

		let sequence = slot.sequence.load(Ordering::Relaxed);
		let next = self.pos.wrapping_add(1);

		#[allow(clippy::cast_possible_wrap)]
		let diff = sequence.wrapping_sub(next) as isize;

		#[allow(clippy::never_loop)]
		loop {
			if diff != 0 {
				break;
			}

			self.pos = next;

			return Ok(if slot.remaining.fetch_sub(1, Ordering::Relaxed) == 1 {
				/* Safety: this is the last expected receiver */
				unsafe { data.assume_init_read() }
			} else {
				/* Safety: the data is initialized */
				unsafe { data.assume_init_ref() }.clone()
			});
		}

		drop(data);

		if diff < 0 {
			Err(if self.channel.rx_waiters.is_closed() {
				RecvError::Closed
			} else {
				RecvError::Empty
			})
		} else {
			let head = self
				.channel
				.tail
				.load(Ordering::Relaxed)
				.wrapping_sub(self.channel.slots.len());
			let missed = head.wrapping_sub(self.pos);

			self.pos = head;

			Err(RecvError::Lagged(missed))
		}
	}

	/// Receive a value, suspending if the channel is currently empty
	pub async fn recv(&mut self) -> RecvResult<T> {
		let mut backoff = Backoff::new();

		loop {
			let result = self.try_recv();

			if !matches!(result, Err(RecvError::Empty)) || is_interrupted().await {
				return result;
			}

			#[allow(clippy::arithmetic_side_effects)]
			if backoff.is_completed() || !acquire_budget(backoff.step() as u32 + 1).await {
				let should_block = || self.channel.tail.load(Ordering::SeqCst) == self.pos;
				let _ = self.channel.rx_waiters.wait(should_block).await;

				backoff.reset();
			} else {
				backoff.snooze();
			}
		}
	}

	/// Receive a value synchronously, suspending the current thread if the
	/// channel is currently empty. Do **not** call this function from a thread
	/// that is driving async tasks
	///
	/// `should_cancel` is a function that is called when the thread gets
	/// interrupted. If it returns `true`, the operation is signalled to be
	/// cancelled
	///
	/// # Safety
	/// `should_cancel` must never unwind
	pub unsafe fn blocking_recv_cancellable<C>(&mut self, should_cancel: C) -> RecvResult<T>
	where
		C: Fn() -> bool
	{
		let mut backoff = Backoff::new();

		loop {
			let result = self.try_recv();

			if !matches!(result, Err(RecvError::Empty)) || should_cancel() {
				return result;
			}

			if backoff.is_completed() {
				let should_block = || self.channel.tail.load(Ordering::SeqCst) == self.pos;

				/* Safety: guaranteed by caller */
				let _ = unsafe {
					self.channel
						.rx_waiters
						.blocking_wait(should_block, &should_cancel)
				};

				backoff.reset();
			} else {
				backoff.snooze();
			}
		}
	}

	/// A non-cancellable version of [`blocking_recv_cancellable`]. Do **not**
	/// call this function from a thread that is driving async tasks
	///
	/// [`blocking_recv_cancellable`]: Self::blocking_recv_cancellable
	pub fn blocking_recv(&mut self) -> RecvResult<T> {
		/* Safety: function does not unwind */
		unsafe { self.blocking_recv_cancellable(|| false) }
	}
}

impl<T> Drop for Receiver<T> {
	fn drop(&mut self) {
		self.channel.rx_count.fetch_sub(1, Ordering::SeqCst);
	}
}

/// The sender for a broadcast channel
pub struct Sender<T> {
	channel: Arc<Channel<T>>
}

#[asynchronous]
impl<T> Sender<T> {
	fn new(channel: Arc<Channel<T>>) -> Self {
		channel.tx_count.fetch_add(1, Ordering::Relaxed);

		Self { channel }
	}

	/// Send a value to the channel. If the channel is closed, the return value
	/// is an `Err(value)`
	#[allow(clippy::missing_panics_doc, clippy::unwrap_used)]
	pub fn send(&self, value: T) -> SendResult<T> {
		let mask = self.channel.mask();
		let index = self.channel.tail.fetch_add(1, Ordering::SeqCst);
		let receivers = self.channel.rx_count.load(Ordering::SeqCst);

		/* Safety: masked */
		let slot = unsafe { self.channel.slots.get_unchecked(index & mask) };
		let mut data = slot.data.write().unwrap();
		let mut previous = None;

		if slot.remaining.swap(receivers, Ordering::Relaxed) != 0 {
			/* Safety: value is still initialized */
			previous = Some(unsafe { data.assume_init_read() });
		}

		slot.sequence
			.store(index.wrapping_add(1), Ordering::Relaxed);

		if receivers == 0 {
			return Err(SendError(value));
		}

		data.write(value);

		drop(data);

		self.channel.rx_waiters.wake_all(());

		drop(previous);

		Ok(())
	}

	/// Create a new [`Receiver`] that receives messages sent after this call
	#[must_use]
	pub fn subscribe(&self) -> Receiver<T>
	where
		T: Clone
	{
		Receiver::new(self.channel.clone())
	}
}

impl<T> Clone for Sender<T> {
	fn clone(&self) -> Self {
		Self::new(self.channel.clone())
	}
}

impl<T> Drop for Sender<T> {
	fn drop(&mut self) {
		if self.channel.tx_count.fetch_sub(1, Ordering::Relaxed) == 1 {
			self.channel.rx_waiters.close(());
		}
	}
}

/// Create a broadcast channel with a capacity of `size`. See [the module
/// documentation](`self`) for more information
#[must_use]
pub fn channel<T: Clone>(size: usize) -> (Sender<T>, Receiver<T>) {
	let channel = Arc::new(Channel::new(size));

	(Sender::new(channel.clone()), Receiver::new(channel))
}
