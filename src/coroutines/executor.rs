#![allow(clippy::multiple_unsafe_ops_per_block)]

use super::*;
use crate::cell::Cell;

/// Per thread executor, responsible for running worker threads
#[cfg_attr(not(any(doc, feature = "xx-doc")), repr(C))]
pub struct Executor {
	current: Cell<Ptr<Worker>>,
	main: Worker,
	pool: Ptr<Pool>
}

impl Executor {
	#[must_use]
	#[allow(clippy::new_without_default)]
	pub fn new() -> Self {
		/* Safety: pool is null */
		unsafe { Self::new_with_pool(Ptr::null()) }
	}

	/// # Safety
	/// `pool` must be either valid for this executor or null
	#[must_use]
	pub unsafe fn new_with_pool(pool: Ptr<Pool>) -> Self {
		Self {
			pool,
			main: Worker::main(),

			/* current is assigned once pinned */
			current: Cell::new(Ptr::null())
		}
	}

	/// # Safety
	/// `pool` must be either valid for this executor or null
	pub unsafe fn set_pool(&mut self, pool: Ptr<Pool>) {
		self.pool = pool;
	}

	/// # Safety
	/// Executor must outlive the worker
	pub unsafe fn new_worker(&self, start: Start) -> Worker {
		if self.pool.is_null() {
			/* Safety: guaranteed by caller */
			unsafe { Worker::new(ptr!(self), start) }
		} else {
			/* Safety: pool must be valid for this executor */
			let fiber = unsafe { ptr!(self.pool=>new_fiber(start)) };

			/* Safety: guaranteed by caller */
			unsafe { Worker::from_fiber(ptr!(self), fiber) }
		}
	}

	/// Workers move themselves onto their own stack when
	/// they get started. We have to update our current reference
	/// when they get moved and pinned
	///
	/// # Safety
	/// the pointer must be the worker that was just started from this executor
	/// and pinned the worker must be alive as long as it's executing
	pub(super) unsafe fn worker_pinned(&self, worker: Ptr<Worker>) {
		self.current.set(worker);
	}

	/// Suspend the current `worker` and resume on the calling worker
	///
	/// # Safety
	/// the passed `worker` must be currently running and started from this
	/// executor
	pub(super) unsafe fn suspend(&self, worker: Ptr<Worker>) {
		/* Safety: worker is valid */
		let worker = unsafe { worker.as_ref() };
		let from = worker.caller();

		/* Safety: a worker cannot be double suspended */
		unsafe { assert_unsafe_precondition!(!from.is_null(), "Double suspend detected") }

		self.current.set(from);

		#[cfg(debug_assertions)]
		/* Safety: clear the caller */
		(unsafe { worker.suspend_to(Ptr::null()) });

		/* Safety: workers are alive as long as they're executing */
		unsafe { Fiber::switch(worker.fiber(), ptr!(from=>fiber())) };
	}

	/// Switch from whichever `current` worker is running to the new `worker`
	///
	/// # Safety
	/// the passed `worker` must not exist on the worker call stack
	/// as workers cannot resume each other recursively.
	///
	/// the worker must be started from this executor
	pub(super) unsafe fn resume(&self, worker: Ptr<Worker>) {
		let previous = self.current.replace(worker);

		/* Safety: worker is valid */
		let worker = unsafe { worker.as_ref() };

		/* Safety: a worker cannot be double resumed */
		unsafe { assert_unsafe_precondition!(worker.caller().is_null(), "Double resume detected") };

		/* Safety: previous resumed worker */
		unsafe { worker.suspend_to(previous) };

		/* Safety: workers must be alive as long as they're executing */
		unsafe { Fiber::switch(ptr!(previous=>fiber()), worker.fiber()) };
	}

	/// Start a new worker
	///
	/// # Safety
	/// same as resume
	/// the worker must be alive until it exits
	pub(super) unsafe fn start(&self, worker: Ptr<Worker>) {
		/* Safety: guaranteed by caller */
		unsafe { self.resume(worker) };
	}

	/// Exit the worker and drop its stack
	///
	/// # Safety
	/// same as suspend
	/// the worker must be finished executing
	pub(super) unsafe fn exit(&self, worker: Worker) {
		let from = worker.caller();

		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(!from.is_null(), "Double suspend detected") };

		self.current.set(from);

		/* Safety: guaranteed by caller */
		unsafe {
			if self.pool.is_null() {
				worker.into_inner().exit(ptr!(from=>fiber()));
			} else {
				worker
					.into_inner()
					.exit_to_pool(ptr!(from=>fiber()), self.pool);
			}
		}
	}
}

impl Pin for Executor {
	unsafe fn pin(&mut self) {
		self.current.set(ptr!(&self.main));
	}
}
