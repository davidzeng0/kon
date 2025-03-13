use crate::error::*;
use crate::fiber::*;
use crate::future::internal::*;
use crate::future::{self, future, Cancel, Complete, Future, Progress, ReqPtr, Request};
use crate::macros::{assert_unsafe_precondition, unreachable_unchecked};
pub use crate::macros::{asynchronous, join, select};
use crate::opt::hint::*;
use crate::pointer::*;
use crate::runtime::{self, call_no_unwind, catch_unwind_safe, MaybePanic};
use crate::{debug, trace, warn};

mod lang {}

pub mod branch;
pub mod context;
pub mod environment;
pub mod executor;
pub mod impls;
pub mod join;
pub mod ops;
pub mod select;
pub mod spawn;
pub mod wake;
pub mod worker;

pub mod internal;

#[doc(inline)]
pub use {
	context::*, environment::*, executor::*, join::*, select::*, spawn::*, wake::*, worker::*
};

use self::branch::*;
use self::ops::*;

/// The budget for async tasks
pub const DEFAULT_BUDGET: u32 = 128;

/// A task represents an asynchronous computation obtained by use of `async`.
///
/// An async task is a value that might not have finished computing yet. This
/// kind of "asynchronous value" makes it possible for a thread to continue
/// doing useful work while it waits for the value to become available
#[asynchronous]
#[lang = "task"]
#[must_use = "Task does nothing until you `.await` it"]
pub trait Task {
	/// The type of value produced on completion
	type Output;

	/// An async task is not started until the caller `.await`s the
	/// value. This function runs the task and returns its output. If the result
	/// is not ready immediately, then the current execution context is
	/// suspended and only resumed once it's ready to make progress.
	async fn run(self) -> Self::Output;
}

/// Get the current async context
///
/// `'current` is a lifetime that is only valid for the current async function
///
/// The context allows an async task to suspend execution while waiting for an
/// operation to complete, and is passed in a call to [`Task::run`].
///
/// See also [`scoped`]
///
/// To return a lifetime referencing the context, add `#[cx]` to the lifetime
/// you wish to use as the lifetime of the context
///
/// ```
/// #[asynchronous]
/// async fn borrows_context<#[cx] 'current>() -> &'current Context {
/// 	get_context().await
/// }
/// ```
#[asynchronous]
#[lang = "get_context"]
pub async fn get_context() -> &'current Context {
	/* compiler builtin */
}

/// Runs the async task `T` on the current execution context, as if the caller
/// were an async function `.await`ing a result
///
/// This is useful when interfacing with synchronous code without needing to
/// block the current thread by using the async context to prevent blocking
///
/// # Safety
/// The current execution context (aka call stack and instruction pointer) must
/// belong to the async context passed into this function. It is an error to
/// export the context to another async worker or thread and call this function
///
/// The async task `T` may suspend the current execution context when waiting
/// for an operation to complete. The caller must ensure that either the task
/// never suspends, or if it does, that all references are allowed to cross the
/// await barrier.
///
/// Note that references are still borrowed even when suspended. This means that
/// they must not become dangling, or violate the aliasing rules if another
/// worker acquires a reference to the same data
pub unsafe fn scoped<T, Output>(context: &Context, task: T) -> Output
where
	T: for<'ctx> Task<Output<'ctx> = Output>
{
	#[cfg(any(doc, feature = "xx-doc"))]
	unreachable!();

	#[cfg(not(any(doc, feature = "xx-doc")))]
	/* Safety: guaranteed by caller */
	(unsafe { context.run(task) })
}

/// Block on a future `F`, suspending until it completes
///
/// Async functions act like sync functions until it's time to suspend and wait
/// for an operation to complete. The async function is then resumed when the
/// future completes.
///
/// [`block_on`] and [`block_on_thread_safe`] are the only places where async
/// functions suspend.
///
/// See [`Future`] for more information
#[asynchronous]
#[inline]
pub async fn block_on<F>(future: F) -> F::Output
where
	F: Future
{
	/* Safety: we are in an async function */
	unsafe { get_context().await.block_on(future, false) }
}

/// Runs and blocks on future `F` which is expected to be completed from
/// another thread
///
/// See [`block_on`]
///
/// # Panics
/// If the operation isn't supported by the current async runtime
#[asynchronous]
#[inline]
pub async fn block_on_thread_safe<F>(future: F) -> F::Output
where
	F: Future
{
	/* Safety: we are in an async function */
	unsafe { get_context().await.block_on(future, true) }
}

/// Get the remaining budget for the current async worker
///
/// The budget limits the amount of work an async worker can perform to prevent
/// starving other workers.
#[asynchronous]
pub async fn current_budget() -> u32 {
	get_context().await.current_budget() as u32
}

/// Attempt to acquire some budget from the current worker's remaining budget to
/// do async work
///
/// Returns whether or not the budget was successfully acquired.
///
/// Note that this function does not do any suspending itself. It is up to the
/// caller to suspend if this function returns `false`.
#[asynchronous]
#[allow(clippy::impl_trait_in_params)]
pub async fn acquire_budget(amount: impl Into<Option<u32>>) -> bool {
	let amount = amount.into().unwrap_or(1).try_into().unwrap_or(u16::MAX);

	get_context().await.decrease_budget(amount).is_some()
}

/// An async worker that is being cancelled is in an interrupted state.
///
/// Most I/O and blocking operations like timers, locking a mutex, or receiving
/// a value on a channel will fail when interrupted.
///
/// An async worker should exit as soon as possible when it is interrupted.
#[asynchronous]
pub async fn is_interrupted() -> bool {
	get_context().await.interrupted()
}

/// See [`is_interrupted`]
///
/// If the current worker is interrupted, returns an interrupted error
#[asynchronous]
pub async fn check_interrupt() -> Result<()> {
	if !is_interrupted().await {
		Ok(())
	} else {
		Err(ErrorKind::Interrupted.into())
	}
}

/// Removes an active interrupt on this worker, if any
///
/// I/O and blocking operations will work again after this call, but if the
/// current worker is being cancelled, it should exit as soon as possible. This
/// function should only be called if the current worker should continue running
/// instead.
///
/// If an asynchronous cleanup has to happen before this worker exits, use an
/// [`interrupt_guard`] instead.
#[asynchronous]
pub async fn clear_interrupt() {
	get_context().await.clear_interrupt();
}

/// Returns whether or not the current worker is interrupted.
///
/// If so, the interrupt is cleared.
///
/// See [`clear_interrupt`]
#[asynchronous]
pub async fn take_interrupt() -> bool {
	let interrupted = is_interrupted().await;

	if interrupted {
		clear_interrupt().await;
	}

	interrupted
}

/// Returns an error if the current worker is interrupted.
///
/// If so, the interrupt is cleared.
///
/// See [`check_interrupt`] and [`take_interrupt`]
#[asynchronous]
pub async fn check_interrupt_take() -> Result<()> {
	if !take_interrupt().await {
		Ok(())
	} else {
		Err(ErrorKind::Interrupted.into())
	}
}

/// Creates an interrupt guard
///
/// While this guard is held, any attempt to interrupt
/// the current worker will be ignored
///
/// If the current worker is already interrupted, [`is_interrupted`] will now
/// return `false` until all guards are dropped, allowing I/O and blocking
/// operations to proceed
#[asynchronous]
pub async fn interrupt_guard<#[cx] 'current>() -> InterruptGuard<'current> {
	InterruptGuard::new(get_context().await)
}
