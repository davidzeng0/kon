//! The building blocks of any asynchronous operation

use crate::error::*;
pub use crate::macros::future;
use crate::pointer::*;
use crate::runtime::call_no_unwind;

mod block_on;

#[doc(hidden)]
pub mod internal;

#[doc(inline)]
pub use block_on::*;

pub type ReqPtr<T> = Ptr<Request<T>>;
pub type Complete<T> = unsafe fn(ReqPtr<T>, Ptr<()>, T);

/// A request contains the information required to inform the caller that a
/// future has completed.
pub struct Request<T> {
	/// The user data to be passed back in the callback.
	pub arg: Ptr<()>,

	/// The callback to run when the future completes.
	pub callback: Complete<T>
}

impl<T> Clone for Request<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T> Copy for Request<T> {}

/// Safety: changing the values of a request after starting a future is
/// discouraged and can result in undefined behavior.
///
/// See [`Future`] for more information
unsafe impl<T> Send for Request<T> {}

/// Safety: changing the values of a request after starting a future is
/// discouraged and can result in undefined behavior.
///
/// See [`Future`] for more information
unsafe impl<T> Sync for Request<T> {}

impl<T> Request<T> {
	pub const fn no_op() -> Self {
		fn no_op<T>(_: ReqPtr<T>, _: Ptr<()>, _: T) {}

		/* Safety: no_op does not unwind */
		unsafe { Self::new(Ptr::null(), no_op) }
	}

	/// Create a new request with a user pointer and a callback
	///
	/// The callback is called once when the future completes.
	///
	/// The callback may be executed from another thread. See documentation for
	/// the specific future for more information.
	///
	/// # Safety
	/// `callback` must not unwind
	pub const unsafe fn new(arg: Ptr<()>, callback: Complete<T>) -> Self {
		Self { arg, callback }
	}

	pub fn set_arg(&mut self, arg: Ptr<()>) {
		self.arg = arg;
	}

	/// # Safety
	/// must not call after future already completed, and must not call within
	/// `Future::run`
	///
	/// Whether or not it is safe to complete a `Future` from a different
	/// thread is defined by the `Future` implementation
	pub unsafe fn complete(request: Ptr<Self>, value: T) {
		/* Safety: guaranteed by caller and Future's contract */
		let Self { arg, callback } = unsafe { ptr!(*request) };

		/* Safety: guaranteed by caller */
		call_no_unwind(|| unsafe { (callback)(request, arg, value) });
	}
}

/// A cancel token for cancelling an in progress future
///
/// Cancelling is on a best-effort basis. If the cancellation fails, the user
/// should ignore the error and hope that the future completes in a reasonable
/// amount of time. Unless running critically low on memory, or some other
/// extreme edge case, cancellations should always succeed. Even if the cancel
/// operation returns `Ok(())`, that does not mean a cancel was successful,
/// because cancellations may be asynchronous.
///
/// After initiating a cancel, you must wait for the callback to be called
/// before releasing any data passed into the future, including the request
///
/// # Safety
/// Implementers must not expect captured references to live until the cancel
/// operation itself finishes, only until the callback is called.
///
/// If [`Cancel::run`] panics, the callback must not be called from within, and
/// the future is assumed to still be in progress
pub unsafe trait Cancel {
	/// Cancel the future
	///
	/// It is possible that the callback is immediately executed in the call to
	/// cancel.
	///
	/// # Safety
	/// The future must be in progress and this function cannot be called more
	/// than once. The caller must be prepared for the callback to be executed
	/// from within this call
	unsafe fn run(self) -> Result<()>;
}

/// The state of the operation after a call to [`Future::run`].
#[must_use]
pub enum Progress<Output, C> {
	/// The operation is pending
	/// The callback on the request will be called when it is complete
	Pending(C),

	/// The operation completed synchronously
	/// The callback on the request will not be called
	Done(Output)
}

/// A [`Future`] is a primitive for an asynchronous operation. (Not to be
/// confused with [`std::future::Future`])
///
/// When a future is created, it contains all the necessary information to
/// perform an operation.
///
/// The future is started with a call to [`Future::run`]. A request is passed to
/// this function as a callback for when the operation completes.
///
/// The future may complete immediately, in which case the callback will not be
/// called. Otherwise, a [`Cancel`] token is returned to allow the caller to
/// request the operation be cancelled.
///
/// Each request pointer should be unique, as it may be possible that
/// only one future can be started for each request. Using the same request
/// pointer for two in progress futures is unspecified behavior.
///
/// Some implementations may use the pointer as the key for [`Cancel::run`],
/// which will cancel at least one future with the same key
///
/// # Safety
/// Any pointers and references passed to the future by the caller must only
/// live while the operation is active. Once the implementation executes the
/// callback, they must not be accessed.
///
///
/// If [`Future::run`] panics, the future is considered completed, implementers
/// must no longer use any pointers or references from the caller, and the
/// callback must not be called
///
/// If the future can be constructed with raw pointers and integers in safe
/// code, the constructor must be marked as unsafe, as safe blocking
/// functions cannot guarantee all lifetimes are captured and live for the
/// blocking duration
#[must_use = "Future does nothing until `Future::run` is called"]
pub unsafe trait Future {
	type Output;
	type Cancel: Cancel;

	/// Run the future.
	///
	/// The `request` contains a user data pointer and a callback for when the
	/// future completes. Changing the values after a future is started is
	/// unspecified behavior and discouraged. If the request is completed from
	/// another thread, it may result in undefined behavior.
	///
	/// # Safety
	/// The caller is responsible for ensuring all pointers/references passed
	/// to the future implementer stays valid until the callback is called
	unsafe fn run(self, request: ReqPtr<Self::Output>) -> Progress<Self::Output, Self::Cancel>;
}
