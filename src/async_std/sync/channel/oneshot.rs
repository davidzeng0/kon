#![allow(clippy::needless_pass_by_ref_mut)]
//! Sends a single message to another task.
//!
//! Create a channel using the [`channel()`] function, which returns a
//! [`Sender`] and [`Receiver`] pair used to send and receive the value,
//! respectively.
//!
//! # Example
//!
//! ```
//! let (tx, rx) = oneshot::channel();
//!
//! spawn(async move {
//! 	println!("{:?}", rx.await);
//! })
//! .await;
//!
//! tx.send("hello world");
//! ```
//!
//! If the `Sender` is dropped before a message is sent, the receiver returns
//! with a [`RecvError::Closed`] error. If the `Receiver` is dropped before a
//! message is sent, [`Sender::send`] fails with `Err(value)`
//!
//! For robust feedback, the receiving end should close the channel first and
//! try a recv before dropping the receiver.
//!
//! ```
//! let mut rx = ...;
//!
//! rx.close();
//!
//! let result = rx.try_recv();
//! ```
//!
//! This guarantees that either the receiving end received the value, or the
//! value be returned as an error on the sending side

pub use super::error::RecvError;
use super::*;

struct Channel<T> {
	sent: AtomicBool,
	value: UnsafeCell<MaybeUninit<T>>,
	tx_waiter: AtomicWaiter,
	rx_waiter: AtomicWaiter
}

impl<T> Channel<T> {
	const fn new() -> Self {
		Self {
			sent: AtomicBool::new(false),
			value: UnsafeCell::new(MaybeUninit::uninit()),
			tx_waiter: AtomicWaiter::new(),
			rx_waiter: AtomicWaiter::new()
		}
	}

	#[allow(clippy::multiple_unsafe_ops_per_block)]
	fn try_consume_value(&self) -> Option<T> {
		self.sent
			.swap(false, Ordering::SeqCst)
			/* Safety: we took ownership of the value */
			.then(|| unsafe { ptr!(self.value=>assume_init_read()) })
	}

	fn close(&self) {
		self.tx_waiter.close(());
		self.rx_waiter.close(());
	}
}

/* Safety: only if T is send */
unsafe impl<T: Send> Send for Channel<T> {}

/* Safety: only if T is send */
unsafe impl<T: Send> Sync for Channel<T> {}

/// The receiver of a oneshot channel
pub struct Receiver<T> {
	channel: Arc<Channel<T>>
}

#[asynchronous]
impl<T> Receiver<T> {
	/// Attempt to receive a value in the channel without suspending
	pub fn try_recv(&mut self) -> RecvResult<T> {
		match self.channel.try_consume_value() {
			Some(value) => {
				self.channel.tx_waiter.close(());

				Ok(value)
			}

			None => Err(RecvError::new(self.channel.tx_waiter.is_closed()))
		}
	}

	/// Receive a value, suspending if the channel is currently empty
	///
	/// # Cancel safety
	///
	/// This function is cancel safe. Call this function again to resume the
	/// operation.
	pub async fn recv(&mut self) -> RecvResult<T> {
		let _ = self.channel.rx_waiter.wait_thread_safe().await;

		self.try_recv()
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
		/* Safety: both functions never unwind */
		let _ = unsafe { self.channel.rx_waiter.blocking_wait(|| true, should_cancel) };

		self.try_recv()
	}

	/// A non-cancellable version of [`blocking_recv_cancellable`]. Do **not**
	/// call this function from a thread that is driving async tasks
	///
	/// [`blocking_recv_cancellable`]: Self::blocking_recv_cancellable
	pub fn blocking_recv(&mut self) -> RecvResult<T> {
		/* Safety: function does not unwind */
		unsafe { self.blocking_recv_cancellable(|| false) }
	}

	/// Close the channel, preventing a later call to `send` from succeeding
	pub fn close(&mut self) {
		self.channel.close();
	}
}

#[asynchronous(task)]
impl<T> Task for Receiver<T> {
	type Output = RecvResult<T>;

	/// Consumes the `Receiver` and asynchronously receives a value from the
	/// channel
	async fn run(mut self) -> Self::Output {
		self.recv().await
	}
}

impl<T> Drop for Receiver<T> {
	fn drop(&mut self) {
		self.channel.tx_waiter.close(());

		/* the value may already be sent. try a read here to prevent leaking */
		let _ = self.channel.try_consume_value();
	}
}

/// The sender of a oneshot channel
pub struct Sender<T> {
	channel: Arc<Channel<T>>
}

#[asynchronous]
impl<T> Sender<T> {
	/// Send a value to the channel. If the channel is closed, the return value
	/// is an `Err(value)`
	pub fn send(self, value: T) -> result::Result<(), T> {
		/* Safety: first time storing a value */
		unsafe { ptr!(self.channel.value=>write(value)) };

		self.channel.sent.store(true, Ordering::SeqCst);
		self.channel.rx_waiter.close(());

		if !self.channel.tx_waiter.is_closed() {
			return Ok(());
		}

		/* the receiver closed, so they may not receive the value. try a read here to
		 * prevent leaking
		 */
		match self.channel.try_consume_value() {
			None => Ok(()),
			Some(value) => Err(value)
		}
	}

	/// Check if the channel is closed. Note that it is a race condition to do
	/// the following
	///
	/// ```
	/// let mut tx = ...;
	///
	/// if !tx.is_closed() {
	/// 	tx.send(value).unwrap();
	/// }
	/// ```
	///
	/// Instead, handle the error returned by `send`
	#[must_use]
	pub fn is_closed(&self) -> bool {
		self.channel.tx_waiter.is_closed()
	}

	/// Wait for the channel to be closed. This is useful to stop an in progress
	/// computation if the receiver no longer needs the value
	///
	/// This can return `false` if the current task is interrupted
	///
	/// # Cancel safety
	///
	/// This function is cancel safe. Once the interrupt has been cleared, call
	/// this function again to resume the operation.
	pub async fn closed(&mut self) -> bool {
		let _ = self.channel.tx_waiter.wait_thread_safe().await;

		self.is_closed()
	}
}

impl<T> Drop for Sender<T> {
	fn drop(&mut self) {
		self.channel.close();
	}
}

/// Create a oneshot channel. See [the module documentation](`self`) for more
/// information
#[must_use]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
	let channel = Arc::new(Channel::new());

	(Sender { channel: channel.clone() }, Receiver { channel })
}
