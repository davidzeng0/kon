use super::*;

/// The environment for an async worker
///
/// # Safety
/// implementations must obey the contracts for implementing the functions
pub unsafe trait Environment: 'static {
	/// Gets the context associated with the worker
	///
	/// This function must never unwind, and must return the same context every
	/// time
	fn context(&self) -> &Context;

	/// Gets the context associated with the worker
	///
	/// This function must never unwind, and must return the same context as
	/// `Environment::context`
	fn context_mut(&mut self) -> &mut Context;

	/// Returns the Environment that owns the Context
	///
	/// This function must never unwind
	///
	/// # Safety
	/// the context must be the one contained in this env
	unsafe fn from_context(context: &Context) -> &Self;

	/// Creates a new environment for a new worker
	///
	/// # Safety
	/// the environment and the contained context must be alive while it's
	/// executing this function is unsafe so that Context::run doesn't need
	/// these guarantees
	unsafe fn clone(&self) -> Self;

	/// Returns the executor
	///
	/// The executor must be a valid pointer
	/// This function must never unwind
	fn executor(&self) -> Ptr<Executor>;

	/// Manually suspend the worker
	///
	/// # Safety
	/// See Worker::suspend
	unsafe fn suspend(&self) {
		/* Safety: guaranteed by caller */
		unsafe { self.context().suspend() };
	}

	/// Manually resume the worker
	///
	/// # Safety
	/// See Worker::resume
	unsafe fn resume(&self) {
		/* Safety: guaranteed by caller */
		unsafe { self.context().resume() };
	}
}
