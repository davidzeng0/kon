use std::fmt::{Debug, Formatter, Result};
use std::ops::{Deref, DerefMut};
use std::result;

use super::*;
use crate::cell::UnsafeCell;
use crate::macros::errors;

#[errors]
pub enum TryLockError {
	#[display("Try lock failed because the operation would block")]
	WouldBlock
}

pub struct SpinMutexGuard<'a, T: ?Sized> {
	lock: &'a SpinMutex<T>
}

impl<'a, T: ?Sized> SpinMutexGuard<'a, T> {
	/// # Safety
	/// caller must have the lock held
	const unsafe fn new(lock: &'a SpinMutex<T>) -> Self {
		Self { lock }
	}
}

impl<T: ?Sized> Deref for SpinMutexGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		/* Safety: lock held */
		unsafe { self.lock.value.as_ref() }
	}
}

impl<T: ?Sized> DerefMut for SpinMutexGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		/* Safety: lock held */
		unsafe { self.lock.value.as_mut() }
	}
}

impl<T: ?Sized> Drop for SpinMutexGuard<'_, T> {
	fn drop(&mut self) {
		self.lock.lock.unlock();
	}
}

impl<T: ?Sized + Debug> Debug for SpinMutexGuard<'_, T> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		(**self).fmt(fmt)
	}
}

/* Safety: same as &mut T */
unsafe impl<T: ?Sized + Send> Send for SpinMutexGuard<'_, T> {}

/* Safety: same as &mut T */
unsafe impl<T: ?Sized + Sync> Sync for SpinMutexGuard<'_, T> {}

pub struct SpinMutex<T: ?Sized> {
	lock: SpinLock,
	value: UnsafeCell<T>
}

impl<T: ?Sized> SpinMutex<T> {
	pub const fn new(value: T) -> Self
	where
		T: Sized
	{
		Self {
			lock: SpinLock::new(),
			value: UnsafeCell::new(value)
		}
	}

	pub fn lock(&self) -> SpinMutexGuard<'_, T> {
		self.lock.lock();

		/* Safety: guaranteed by caller */
		unsafe { SpinMutexGuard::new(self) }
	}

	pub fn try_lock(&self) -> result::Result<SpinMutexGuard<'_, T>, TryLockError> {
		if self.lock.try_lock() {
			/* Safety: guaranteed by caller */
			let guard = unsafe { SpinMutexGuard::new(self) };

			Ok(guard)
		} else {
			Err(TryLockError::WouldBlock)
		}
	}

	pub fn into_inner(self) -> T
	where
		T: Sized
	{
		self.value.into_inner()
	}

	pub fn get_mut(&mut self) -> &mut T {
		self.value.get_mut()
	}
}

/* Safety: a mutex is send if T is send */
unsafe impl<T: ?Sized + Send> Send for SpinMutex<T> {}

/* Safety: a mutex is sync if T is send */
unsafe impl<T: ?Sized + Send> Sync for SpinMutex<T> {}
