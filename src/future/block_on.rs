#![allow(clippy::missing_safety_doc)]

use std::mem::replace;
#[cfg(not(debug_assertions))]
use std::mem::ManuallyDrop;

use super::*;
#[cfg(debug_assertions)]
use crate::macros::unreachable_unchecked;

#[cfg(debug_assertions)]
enum BlockState<Resume, Output> {
	Pending(Resume),
	Done(Output)
}

#[cfg(debug_assertions)]
impl<Resume, Output> BlockState<Resume, Output> {
	const fn pending(resume: Resume) -> Self {
		Self::Pending(resume)
	}

	unsafe fn complete(&mut self, value: Output) -> Resume {
		let resume = replace(self, Self::Done(value));

		match resume {
			Self::Pending(resume) => resume,

			/* Safety: future cannot complete twice */
			Self::Done(_) => unsafe { unreachable_unchecked!("Double complete detected") }
		}
	}

	unsafe fn output(self) -> Output {
		match self {
			Self::Done(output) => output,

			/* Safety: guaranteed by caller */
			Self::Pending(_) => unsafe {
				unreachable_unchecked!("Blocking function ended before producing a result")
			}
		}
	}
}

#[cfg(not(debug_assertions))]
union BlockState<Resume, Output> {
	pending: ManuallyDrop<Resume>,
	done: ManuallyDrop<Output>
}

#[cfg(not(debug_assertions))]
impl<Resume, Output> BlockState<Resume, Output> {
	const fn pending(resume: Resume) -> Self {
		Self { pending: ManuallyDrop::new(resume) }
	}

	unsafe fn complete(&mut self, value: Output) -> Resume {
		let resume = replace(self, Self { done: ManuallyDrop::new(value) });

		/* Safety: guaranteed by caller */
		ManuallyDrop::into_inner(unsafe { resume.pending })
	}

	const unsafe fn output(self) -> Output {
		/* Safety: guaranteed by caller */
		ManuallyDrop::into_inner(unsafe { self.done })
	}
}

/// # Safety
/// See [`Request::complete`]
unsafe fn block_resume<Resume, Output>(_: ReqPtr<Output>, arg: Ptr<()>, value: Output)
where
	Resume: FnOnce()
{
	/* Safety: arg is valid until resume is called */
	let arg = unsafe { arg.cast::<BlockState<Resume, Output>>().cast_mut().as_mut() };

	/* Safety: guaranteed by caller */
	let resume = unsafe { arg.complete(value) };

	call_no_unwind(resume);
}

struct Waiter<Resume, Output> {
	request: Request<Output>,
	state: BlockState<Resume, Output>
}

impl<Resume: FnOnce(), Output> Waiter<Resume, Output> {
	const fn new(resume: Resume) -> Self {
		/* Safety: block_resume does not unwind */
		let request = unsafe { Request::new(Ptr::null(), block_resume::<Resume, Output>) };

		Self { request, state: BlockState::pending(resume) }
	}

	/// # Safety
	/// future must be complete
	#[allow(clippy::missing_const_for_fn)]
	unsafe fn output(self) -> Output {
		/* Safety: guaranteed by caller */
		unsafe { self.state.output() }
	}
}

impl<Resume, Output> Pin for Waiter<Resume, Output> {
	unsafe fn pin(&mut self) {
		self.request
			.set_arg(ptr!(&mut self.state).cast_const().cast());
	}
}

/// Block on a future, waiting for it to complete
///
/// `block` is called with the future's cancel handle when the future is in
/// progress
///
/// `resume` is a function that is called when the future finishes,
/// to signal to the `block`ing function that it should return
///
/// # Safety
/// `block` must block until the future finishes. It is safe to unwind after
/// the future finishes, but may result in a memory leak
///
/// `resume` must never unwind
///
/// `resume` may be called from another thread. The specifics depend on the
/// future implementation
pub unsafe fn block_on<Block, Resume, F>(block: Block, resume: Resume, future: F) -> F::Output
where
	Block: FnOnce(F::Cancel),
	Resume: FnOnce(),
	F: Future
{
	let mut waiter = Waiter::new(resume);

	{
		/* Safety: waiter is never moved */
		let waiter = unsafe { waiter.pin_local() };

		/* Safety: contract upheld by caller */
		unsafe {
			match future.run(ptr!(&waiter.request)) {
				Progress::Pending(cancel) => block(cancel),
				Progress::Done(value) => return value
			};
		};
	}

	/* Safety: block must block until the future is complete */
	unsafe { waiter.output() }
}

#[cfg(feature = "os")]
#[allow(clippy::missing_panics_doc)]
unsafe fn block_on_sync_impl<F, C>(future: F, should_cancel: C) -> F::Output
where
	F: Future,
	C: Fn() -> bool
{
	use crate::debug;
	use crate::impls::ResultExt;
	use crate::os::futex::Notify;

	let notify = Notify::new();

	pin!(notify);

	let block = |cancel: F::Cancel| {
		let mut cancel = Some(cancel);

		loop {
			#[allow(clippy::unwrap_used)]
			if cancel.is_some() && should_cancel() {
				/* Safety: the future is in progress */
				let result = unsafe { cancel.take().unwrap().run() };

				if let Err(err) = result {
					debug!(">> Cancel failed: {:?}", err);
				}
			}

			/* Safety: pinned */
			let notified = unsafe { notify.wait().expect_nounwind("Block failed") };

			if notified {
				break;
			}
		}
	};

	/* Safety: pinned */
	let resume = || unsafe { notify.notify().expect_nounwind("Notify failed") };

	/* Safety: we are blocked until the future completes */
	unsafe { block_on(block, resume, future) }
}

#[cfg(not(feature = "os"))]
unsafe fn block_on_sync_impl<F, C>(future: F, should_cancel: C) -> F::Output
where
	F: Future,
	C: Fn() -> bool
{
	use std::thread;

	let thread = thread::current();
	let block = |cancel: F::Cancel| {
		if should_cancel() {
			/* Safety: the future is in progress */
			unsafe { cancel.run() };
		}

		thread::park();
	};

	/* Safety: we are blocked until the future completes */
	unsafe { block_on(block, move || thread.unpark(), future) }
}

/// Block the current thread while waiting for a future to complete. The future
/// must be completed from another thread, as the current thread is unable to
/// make any progress while waiting
///
/// `should_cancel` is a function that is called when the thread gets
/// interrupted. If it returns `true`, the future is signalled to be cancelled
///
/// # Safety
/// `should_cancel` must never unwind
pub unsafe fn block_on_sync<F, C>(future: F, should_cancel: C) -> F::Output
where
	F: Future,
	C: Fn() -> bool
{
	/* Safety: guaranteed by caller */
	unsafe { block_on_sync_impl(future, should_cancel) }
}
