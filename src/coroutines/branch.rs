#![allow(clippy::multiple_unsafe_ops_per_block, unreachable_pub)]

use std::mem::replace;

use super::*;
use crate::cell::Cell;
use crate::impls::OptionExt;

/// # Safety
/// the future must be in progress
unsafe fn run_cancel<C>(cancel: C) -> MaybePanic<Result<()>>
where
	C: Cancel
{
	/* Safety: contract upheld by caller */
	let result = catch_unwind_safe(|| unsafe { cancel.run() });

	match &result {
		Ok(Err(err)) => debug!("Cancel failed: {:?}", err),
		Err(_) => warn!("Cancel panicked"),
		_ => ()
	}

	result
}

enum State<F: Future> {
	Ready(F),
	Pending(F::Cancel),
	Done(MaybePanic<F::Output>),
	Empty
}

impl<F: Future> State<F> {
	const fn done(&self) -> bool {
		matches!(self, Self::Done(_))
	}

	fn take(&mut self) -> Self {
		replace(self, Self::Empty)
	}

	fn take_cancel(&mut self) -> Option<F::Cancel> {
		match self.take() {
			Self::Pending(cancel) => Some(cancel),

			value => {
				*self = value;

				None
			}
		}
	}
}

struct FutureHandle<F: Future> {
	state: State<F>,
	request: Request<F::Output>
}

impl<F: Future> FutureHandle<F> {
	/// # Safety
	/// See `Request::new`
	unsafe fn new(future: F, callback: Complete<F::Output>) -> Self {
		Self {
			state: State::Ready(future),

			/* Safety: guaranteed by caller */
			request: unsafe { Request::new(Ptr::null(), callback) }
		}
	}

	fn set_arg(&mut self, arg: Ptr<()>) {
		self.request.set_arg(arg);
	}

	const fn done(&self) -> bool {
		self.state.done()
	}

	/// # Safety
	/// See `Future::run`
	/// Must not call twice
	/// Must call `FutureHandle::complete` when the future finishes
	/// Request arg must be set
	unsafe fn run(&mut self) -> Option<&mut MaybePanic<F::Output>> {
		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(!self.request.arg.is_null()) };

		let future = match self.state.take() {
			State::Ready(future) => future,

			/* Safety: guaranteed by caller */
			_ => unsafe { unreachable_unchecked!("`FutureHandle::run` called twice") }
		};

		/* Safety: guaranteed by caller */
		let progress = catch_unwind_safe(|| unsafe { future.run(ptr!(&self.request)) });

		match progress {
			Ok(Progress::Pending(cancel)) => {
				self.state = State::Pending(cancel);

				None
			}

			Ok(Progress::Done(result)) => {
				/* Safety: the future completed synchronously */
				Some(unsafe { self.complete(Ok(result)) })
			}

			Err(err) => {
				/* Safety: the future panicked */
				Some(unsafe { self.complete(Err(err)) })
			}
		}
	}

	/// # Safety
	/// must only call once, when the future is finished
	unsafe fn complete(&mut self, result: MaybePanic<F::Output>) -> &mut MaybePanic<F::Output> {
		/* Safety: guaranteed by caller */
		unsafe {
			assert_unsafe_precondition!(!self.done(), "`FutureHandle::complete` called twice");
		}

		self.state = State::Done(result);

		match &mut self.state {
			State::Done(value) => value,
			/* Safety: we just stored a Done */
			_ => unsafe { unreachable_unchecked() }
		}
	}

	/// # Safety
	/// the future must have completed, and `FutureHandle::complete` was called
	/// with the result
	unsafe fn result(&mut self) -> MaybePanic<F::Output> {
		match self.state.take() {
			State::Done(value) => value,

			/* abort here (on debug) because this is most definitely fatal, and is usually called
			 * within Request::complete
			 *
			 * Safety: guaranteed by caller
			 */
			_ => unsafe {
				unreachable_unchecked!(
					"Fatal error: called `FutureHandle::result` on an in progress future"
				)
			}
		}
	}

	fn take_cancel(&mut self) -> Option<F::Cancel> {
		self.state.take_cancel()
	}

	/// # Safety
	/// the future must be in progress
	/// there must be no references to Self
	/// `this` must be a valid pointer
	/// `this` may become dangling after the function call
	unsafe fn cancel(this: MutPtr<Self>) -> MaybePanic<Result<()>> {
		/* Safety: guaranteed by caller */
		unsafe {
			let cancel =
				ptr!(this=>take_cancel()).expect_unchecked("`FutureHandle::cancel` called twice");

			run_cancel(cancel)
		}
	}
}

pub struct BranchOutput<O1, O2>(
	pub bool,
	pub Option<MaybePanic<O1>>,
	pub Option<MaybePanic<O2>>
);

impl<O1, O2> BranchOutput<MaybePanic<O1>, MaybePanic<O2>> {
	pub fn flatten(self) -> BranchOutput<O1, O2> {
		fn flatten<T>(result: MaybePanic<MaybePanic<T>>) -> MaybePanic<T> {
			match result {
				Ok(result) => result,
				Err(result) => Err(result)
			}
		}

		BranchOutput(self.0, self.1.map(flatten), self.2.map(flatten))
	}
}

pub struct Branch<F1: Future, F2: Future, Cancel> {
	handles: (FutureHandle<F1>, FutureHandle<F2>),
	request: ReqPtr<BranchOutput<F1::Output, F2::Output>>,
	should_cancel: Cancel,
	interrupt_callback: Cell<bool>
}

impl<
		F1: Future,
		F2: Future,
		C1: Fn(&MaybePanic<F1::Output>) -> bool,
		C2: Fn(&MaybePanic<F2::Output>) -> bool
	> Branch<F1, F2, (C1, C2)>
{
	/// # Safety
	/// must only be called when a task has completed
	unsafe fn complete_single(this: MutPtr<Self>, is_first: bool, should_cancel: bool) {
		/* Safety: we have mutable access here */
		let this = unsafe { this.as_mut() };

		if this.interrupt_callback.replace(false) {
			return;
		}

		let (first, second) = &mut this.handles;

		if first.done() && second.done() {
			/* Safety: both futures finished */
			let result = unsafe {
				BranchOutput(
					/* reverse order, because this is the last future to complete */
					!is_first,
					Some(first.result()),
					Some(second.result())
				)
			};

			/*
			 * Safety: complete the future. we must not access `self` once a cancel or a
			 * complete is called, as we may be freed by the callee
			 */
			unsafe { Request::complete(this.request, result) };

			return;
		}

		if !should_cancel {
			return;
		}

		/* Safety: if both aren't complete, then the other must be running */
		unsafe {
			/* we can't do much if the cancel panics */
			let _ = if is_first {
				FutureHandle::cancel(ptr!(second))
			} else {
				FutureHandle::cancel(ptr!(first))
			};
		}
	}

	/// # Safety
	/// must only be called when the first task completes
	unsafe fn complete_first(_: ReqPtr<F1::Output>, arg: Ptr<()>, value: F1::Output) {
		let this = arg.cast::<Self>().cast_mut();

		/* Safety: the future has completed */
		let result = unsafe { ptr!(this=>handles.0.complete(Ok(value))) };

		/* Safety: guaranteed by Future's contract */
		let should_cancel = call_no_unwind(|| unsafe { ptr!(this=>should_cancel.0(result)) });

		/* Safety: the future has completed */
		unsafe { Self::complete_single(this, true, should_cancel) };
	}

	/// # Safety
	/// must only be called when the second task completes
	unsafe fn complete_second(_: ReqPtr<F2::Output>, arg: Ptr<()>, value: F2::Output) {
		let this = arg.cast::<Self>().cast_mut();

		/* Safety: the future has completed */
		let result = unsafe { ptr!(this=>handles.1.complete(Ok(value))) };

		/* Safety: guaranteed by Future's contract */
		let should_cancel = call_no_unwind(|| unsafe { ptr!(this=>should_cancel.1(result)) });

		/* Safety: the future has completed */
		unsafe { Self::complete_single(this, false, should_cancel) };
	}

	pub fn new(future_1: F1, future_2: F2, should_cancel: (C1, C2)) -> Self {
		/* Safety: complete does not unwind */
		unsafe {
			/* request args are assigned once pinned */
			Self {
				handles: (
					FutureHandle::new(future_1, Self::complete_first),
					FutureHandle::new(future_2, Self::complete_second)
				),
				request: Ptr::null(),
				should_cancel,
				interrupt_callback: Cell::new(false)
			}
		}
	}

	/// # Safety
	/// See [`Cancel::run`]
	unsafe fn cancel_all(this: MutPtr<Self>) -> Result<()> {
		/* Safety: guaranteed by future's contract */
		let this = unsafe { this.as_mut() };

		/* cancelling directly can lead to UB
		 * future 1: pending
		 * future 2: done
		 *
		 * take and cancel future 1: completes synchronously, completes the branch
		 * take and cancel future 2: use-after-free
		 */
		let cancels = (this.handles.0.take_cancel(), this.handles.1.take_cancel());

		if cancels.0.is_some() && cancels.1.is_some() {
			/* must prevent cancel 1 from trying to call
			 * cancel 2 in callback, which is a None
			 */
			this.interrupt_callback.set(true);
		}

		/* Safety: cancel is None if the future already completed */
		let cancel = unsafe {
			[
				cancels.0.map(|cancel| run_cancel(cancel)),
				cancels.1.map(|cancel| run_cancel(cancel))
			]
		};

		let mut result = Ok(());

		for cancel in cancel {
			let Some(cancel_result) = runtime::join(cancel.transpose()) else {
				continue;
			};

			if result.is_ok() {
				result = cancel_result;
			}
		}

		result
	}

	/// # Safety
	/// see `Future::run`
	/// self must be pinned
	/// `this` must be a valid pointer
	#[future]
	pub unsafe fn run(this_ptr: MutPtr<Self>, request: _) -> BranchOutput<F1::Output, F2::Output> {
		#[cancel]
		fn cancel(self: MutPtr<Self>) -> Result<()> {
			/* Safety: caller must uphold Future's contract */
			unsafe { Self::cancel_all(self) }
		}

		/* Safety: guaranteed by caller */
		let this = unsafe { this_ptr.as_mut() };

		this.request = request;

		/* Safety: caller must uphold Future's contract */
		if let Some(result) = unsafe { this.handles.0.run() } {
			if this.should_cancel.0(result) {
				/* Safety: future completed */
				let result = unsafe { this.handles.0.result() };

				return Progress::Done(BranchOutput(true, Some(result), None));
			}
		}

		/* Safety: caller must uphold Future's contract */
		if let Some(result) = unsafe { this.handles.1.run() } {
			let mut done = this.handles.0.done();

			if !done && this.should_cancel.1(result) {
				this.interrupt_callback.set(true);

				/* Safety: reborrow may occur in cancel */
				unsafe { this.pin() };

				/* Safety: future is in progress */
				let _ = unsafe { FutureHandle::cancel(ptr!(&mut this.handles.0)) };

				/* reborrow has ended */
				if !this.interrupt_callback.replace(false) {
					done = true;
				}
			}

			if done {
				/* Safety: both futures completed */
				let result = unsafe {
					BranchOutput(
						done,
						Some(this.handles.0.result()),
						Some(this.handles.1.result())
					)
				};

				return Progress::Done(result);
			}
		}

		Progress::Pending(cancel(this_ptr))
	}
}

impl<F1: Future, F2: Future, Cancel> Pin for Branch<F1, F2, Cancel> {
	unsafe fn pin(&mut self) {
		let arg = ptr!(&*self);

		self.handles.0.set_arg(arg.cast());
		self.handles.1.set_arg(arg.cast());
	}
}

/// # Safety
/// `should_cancel` must not unwind
#[asynchronous]
pub async unsafe fn branch<F1, F2, C1, C2>(
	future_1: F1, future_2: F2, should_cancel: (C1, C2)
) -> BranchOutput<F1::Output, F2::Output>
where
	F1: Future,
	F2: Future,
	C1: Fn(&MaybePanic<F1::Output>) -> bool,
	C2: Fn(&MaybePanic<F2::Output>) -> bool
{
	let mut branch = Branch::new(future_1, future_2, should_cancel);

	/* Safety: branch is pinned. we are blocked until the future completes */
	block_on(unsafe { Branch::run(ptr!(&mut *branch.pin_local())) }).await
}
