//! The async equivalent of [`std::sync::Mutex`]
//!
//! A mutual exclusion primitive useful for protecting shared data
//!
//! This mutex implements [Poisoning](`std::sync::Mutex#poisoning`)

use std::fmt;
use std::hint::spin_loop;
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::{LockResult, TryLockError, TryLockResult};

use super::*;
use crate::sync::poison::*;

/// The equivalent of a [`std::sync::MutexGuard`]
pub struct MutexGuard<'a, T: ?Sized> {
	lock: &'a Mutex<T>,
	poison: PoisonGuard<'a>
}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
	/// # Safety
	/// must create atmost 1 guards after a successful locking of the mutex
	unsafe fn new(lock: &'a Mutex<T>) -> Self {
		Self { lock, poison: lock.poison.guard() }
	}
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		/* Safety: lock held */
		unsafe { self.lock.value.as_ref() }
	}
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		/* Safety: lock held */
		unsafe { self.lock.value.as_mut() }
	}
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
	fn drop(&mut self) {
		self.poison.finish();

		/* Safety: we own the lock */
		unsafe { self.lock.unlock() };
	}
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'_, T> {
	fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
		(**self).fmt(fmt)
	}
}

/* Safety: same as &mut T */
unsafe impl<T: ?Sized + Send> Send for MutexGuard<'_, T> {}

/* Safety: same as &mut T */
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum State {
	Unlocked,
	Locked,
	Contended
}

/// The async equivalent of [`std::sync::Mutex`]. See [the module
/// documentation](`self`) for more information
pub struct Mutex<T: ?Sized> {
	state: AtomicU8,
	wait_list: ThreadSafeWaitList<()>,
	poison: PoisonFlag,
	value: UnsafeCell<T>
}

#[asynchronous]
impl<T: ?Sized> Mutex<T> {
	/// Create a new mutex in an unlocked state
	pub fn new(value: T) -> Self
	where
		T: Sized
	{
		Self {
			state: AtomicU8::new(State::Unlocked as u8),
			wait_list: ThreadSafeWaitList::new(),
			poison: PoisonFlag::new(),
			value: UnsafeCell::new(value)
		}
	}

	/// # Safety
	/// must have acquired the lock
	unsafe fn unlock(&self) {
		let state = self.state.swap(State::Unlocked as u8, Ordering::SeqCst);

		if state == State::Contended as u8 {
			self.wait_list.wake_one(());
		}
	}

	fn try_lock_internal(&self) -> bool {
		self.state
			.compare_exchange(
				State::Unlocked as u8,
				State::Locked as u8,
				Ordering::Acquire,
				Ordering::Relaxed
			)
			.is_ok()
	}

	fn try_spin_lock(&self) -> u8 {
		let mut state = State::Locked as u8;

		for _ in 0..97 {
			state = self.state.load(Ordering::Relaxed);

			if state != State::Locked as u8 {
				break;
			}

			spin_loop();
		}

		if state != State::Contended as u8 {
			state = self.state.swap(State::Contended as u8, Ordering::Acquire);
		}

		state
	}

	#[cold]
	async fn lock_contended(&self) -> bool {
		loop {
			let prev_state = self.try_spin_lock();

			if prev_state == State::Unlocked as u8 {
				break true;
			}

			if is_interrupted().await {
				break false;
			}

			let should_block = || self.state.load(Ordering::SeqCst) == State::Contended as u8;
			let _ = self.wait_list.wait(should_block).await;
		}
	}

	/// # Safety
	/// must have acquired the lock
	unsafe fn new_guard(&self) -> LockResult<MutexGuard<'_, T>> {
		/* Safety: guaranteed by caller */
		let guard = unsafe { MutexGuard::new(self) };

		self.poison.map(guard)
	}

	/// Acquires the mutex, suspending if it is currently locked. If the current
	/// task is interrupted before the lock could be acquired, a [`WouldBlock`]
	/// error is returned
	///
	/// # Errors
	/// See [`try_lock`]
	///
	/// [`try_lock`]: Self::try_lock
	/// [`WouldBlock`]: TryLockError::WouldBlock
	pub async fn lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
		if !self.try_lock_internal() {
			let locked = self.lock_contended().await;

			if !locked {
				return Err(TryLockError::WouldBlock);
			}
		}

		/* Safety: we acquired the lock */
		unsafe { self.new_guard() }.map_err(Into::into)
	}

	#[cold]
	fn blocking_lock_contended<C>(&self, should_cancel: C) -> bool
	where
		C: Fn() -> bool
	{
		loop {
			let prev_state = self.try_spin_lock();

			if prev_state == State::Unlocked as u8 {
				break true;
			}

			if should_cancel() {
				break false;
			}

			let should_block = || self.state.load(Ordering::SeqCst) == State::Contended as u8;

			/* Safety: guaranteed by caller */
			let _ = unsafe { self.wait_list.blocking_wait(should_block, &should_cancel) };
		}
	}

	/// Lock the mutex synchronously, suspending the current thread if the lock
	/// is already taken. If the current task is interrupted before the lock
	/// could be acquired, a [`WouldBlock`] error is returned. Do **not** call
	/// this function from a thread that is driving async tasks.
	///
	///  `should_cancel` is a function that is called when the thread gets
	/// interrupted. If it returns `true`, the operation is signalled to be
	/// cancelled
	///
	/// # Errors
	/// See [`try_lock`]
	///
	/// # Safety
	/// `should_cancel` must never unwind
	///
	/// [`try_lock`]: Self::try_lock
	/// [`WouldBlock`]: TryLockError::WouldBlock
	pub unsafe fn blocking_lock_cancellable<C>(
		&self, should_cancel: C
	) -> TryLockResult<MutexGuard<'_, T>>
	where
		C: Fn() -> bool
	{
		if !self.try_lock_internal() {
			let locked = self.blocking_lock_contended(should_cancel);

			if !locked {
				return Err(TryLockError::WouldBlock);
			}
		}

		/* Safety: we acquired the lock */
		unsafe { self.new_guard() }.map_err(Into::into)
	}

	/// A non-cancellable version of [`blocking_lock_cancellable`]. Do **not**
	/// call this function from a thread that is driving async tasks
	///
	/// [`blocking_lock_cancellable`]: Self::blocking_lock_cancellable
	pub fn blocking_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
		/* Safety: function does not unwind */
		unsafe { self.blocking_lock_cancellable(|| false) }
	}

	/// Attemps to acquire the lock. If the lock is already locked, returns an
	/// `Err`
	///
	/// # Errors
	/// If the lock was acquired and another user of this mutex panicked with
	/// the lock, a [`Poisoned`] error is returned.
	///
	/// If the mutex could not be acquired because it is already locked, a
	/// [`WouldBlock`] error is returned.
	///
	/// [`Poisoned`]: TryLockError::Poisoned
	/// [`WouldBlock`]: TryLockError::WouldBlock
	pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
		if self.try_lock_internal() {
			/* Safety: guaranteed by caller */
			let guard = unsafe { MutexGuard::new(self) };

			Ok(self.poison.map(guard)?)
		} else {
			Err(TryLockError::WouldBlock)
		}
	}

	/// Returns `true` if the mutex is poisoned
	///
	/// See also [`std::sync::Mutex::is_poisoned`]
	pub fn is_poisoned(&self) -> bool {
		self.poison.get()
	}

	/// Clear the poisoned state from this mutex
	///
	/// See also [`std::sync::Mutex::clear_poison`]
	pub fn clear_poison(&self) {
		self.poison.clear();
	}

	/// Consumes this mutex, returning the underlying data
	pub fn into_inner(self) -> LockResult<T>
	where
		T: Sized
	{
		self.poison.map(self.value.into_inner()).map_err(Into::into)
	}

	/// Returns a mutable reference to the underlying data
	///
	/// See also [`std::sync::Mutex::get_mut`]
	pub fn get_mut(&mut self) -> LockResult<&mut T> {
		self.poison.map(self.value.get_mut()).map_err(Into::into)
	}
}

/* Safety: T is send */
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

/* Safety: T is send. mutex provides synchronization */
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

impl<T: ?Sized> UnwindSafe for Mutex<T> {}

impl<T: ?Sized> RefUnwindSafe for Mutex<T> {}
