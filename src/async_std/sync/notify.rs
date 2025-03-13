//! A simple wait list. There are no synchronization guarantees. A call to
//! `notify` may or may not resume a waiter that is currently in the process of
//! suspending.
//!
//! A task can wait on the list and be resumed optionally with a value.
//!
//! An [`RcNotify`] is slightly more efficient but can only be used in
//! one thread
//!
//! # Example
//!
//! ```
//! let notify = RcNotify::new();
//!
//! spawn(async move {
//! 	notify.wait().await;
//!
//! 	println!("notified");
//! })
//! .await;
//!
//! println!("notifying!");
//!
//! notify.notify(());
//! ```

use std::marker::PhantomData;
use std::rc::Rc;

use super::*;
use crate::macros::wrapper_functions;

/// A raw wait list. See [the module documentation](`self`) for more information
pub struct RawNotify<T = ()> {
	waiters: RawWaitList<T>,
	phantom: PhantomData<T>
}

impl<T: Clone> RawNotify<T> {
	#[asynchronous]
	pub async fn wait(&self) -> Result<T> {
		self.waiters.wait().await.map_err(Into::into)
	}

	pub fn notify(&self, value: T) -> usize {
		self.waiters.wake_all(value)
	}

	/// # Safety
	/// caller must
	/// - pin this Notify
	/// - call Notify::pin
	///
	/// # Unpin
	/// only if waiters is empty
	#[must_use]
	pub const unsafe fn new_unpinned() -> Self {
		Self {
			/* Safety: guaranteed by caller */
			waiters: unsafe { RawWaitList::new() },
			phantom: PhantomData
		}
	}
}

impl<T> Pin for RawNotify<T> {
	unsafe fn pin(&mut self) {
		/* Safety: we are being pinned */
		unsafe { self.waiters.pin() };
	}
}

/// A ref-counted wait list. See [the module documentation](`self`) for more
/// information
pub struct RcNotify<T = ()>(Pinned<Rc<RawNotify<T>>>);

impl<T> Clone for RcNotify<T> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<T: Clone> RcNotify<T> {
	wrapper_functions! {
		inner = self.0;

		#[asynchronous]
		pub async fn wait(&self) -> Result<T>;

		pub fn notify(&self, value: T) -> usize;
	}

	/// Create a new ref-counted notify instance
	#[must_use]
	#[allow(clippy::new_without_default)]
	pub fn new() -> Self {
		/* Safety: cannot be unpinned */
		let raw = unsafe { RawNotify::new_unpinned() };

		Self(raw.pin_rc())
	}
}

/// A thread safe wait list. See [the module documentation](`self`) for more
/// information
pub struct Notify<T = ()> {
	waiters: ThreadSafeWaitList<T>,
	phantom: PhantomData<T>
}

impl<T: Clone> Notify<T> {
	/// Create a new notify instance
	#[must_use]
	pub fn new() -> Self {
		Self {
			/* Safety: guaranteed by caller */
			waiters: ThreadSafeWaitList::new(),
			phantom: PhantomData
		}
	}

	#[asynchronous]
	pub async fn wait(&self) -> Result<T> {
		self.waiters.wait(|| true).await.map_err(Into::into)
	}

	pub fn notify(&self, value: T) -> usize {
		self.waiters.wake_all(value)
	}
}

impl<T: Clone> Default for Notify<T> {
	fn default() -> Self {
		Self::new()
	}
}

/* Safety: T is send */
unsafe impl<T: Send> Send for Notify<T> {}

/* Safety: T is send */
unsafe impl<T: Send> Sync for Notify<T> {}
