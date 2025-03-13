#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::mem::replace;
use std::rc::Rc;

use super::*;
use crate::cell::UnsafeCell;

pub type SpawnResult<T> = MaybePanic<T>;

enum SpawnData<E, T: for<'ctx> Task<Output<'ctx> = Output>, Output> {
	Uninit,
	Start(E, T, Worker, ReqPtr<SpawnResult<Output>>),
	Pending(NonNull<Context>, MutNonNull<bool>),
	Done(SpawnResult<Output>)
}

struct SpawnWorker<E, T: for<'ctx> Task<Output<'ctx> = Output>, Output> {
	data: SpawnData<E, T, Output>
}

#[cfg(not(any(doc, feature = "xx-doc")))]
impl<E: Environment, T: for<'ctx> Task<Output<'ctx> = Output>, Output> SpawnWorker<E, T, Output> {
	/// # Safety
	/// `arg` must be dereferenceable as a &mut Self.
	/// Self::data must be a `SpawnData::Start`
	/// this function must be the entry point of a worker
	unsafe extern "C" fn worker_start(arg: Ptr<()>) {
		let this = arg.cast::<Self>().cast_mut();

		let SpawnData::Start(env, task, mut worker, request) =
			/* Safety: guaranteed by caller */
			replace(unsafe { &mut ptr!(this=>data) }, SpawnData::Uninit)
		else {
			/* Safety: guaranteed by caller */
			unsafe { unreachable_unchecked() };
		};

		trace!(target: &worker, "== Entered worker");

		{
			/* Safety: worker stays pinned while running */
			let worker = unsafe { worker.pin_local() };

			if let Some(sync_output) =
				Self::execute(this, env, ptr!(&*worker), task, request).transpose()
			{
				/* Safety: `this` is still a valid pointer */
				unsafe { ptr!(this=>data = SpawnData::Done(sync_output)) };
			}
		}

		trace!(target: &worker, "== Completed");

		/* Safety: worker has completed */
		unsafe { worker.exit() };
	}

	#[inline(always)]
	fn execute(
		this: MutPtr<Self>, mut env: E, worker: Ptr<Worker>, task: T,
		request: ReqPtr<SpawnResult<Output>>
	) -> SpawnResult<Option<Output>> {
		let context = call_no_unwind(|| env.context_mut());

		/* Safety: worker is valid for this context */
		unsafe { context.set_worker(worker) };

		let mut is_async = false;
		let data = SpawnData::Pending(ptr!(!null & *context), ptr!(!null &mut is_async));

		/* pass back a pointer to is_async. the caller will tell us
		 * if we've suspended from this function
		 *
		 * cannot call Request::complete when synchronous,
		 * as that double resumes the calling worker, and violates the contract of
		 * Future
		 *
		 * Safety: we have mutable access here
		 */
		unsafe { ptr!(this=>data = data) };

		/* Safety: this is an async function */
		let result = catch_unwind_safe(|| unsafe { context.run(task) });

		if is_async {
			/* Safety: only called once
			 * Note: Request::complete does not unwind
			 * Note: `self` is now dangling
			 */
			unsafe { Request::complete(request, result) };

			Ok(None)
		} else {
			result.map(Some)
		}
	}

	/// # Safety
	/// The `env` and `task` must outlive the spawned fiber
	#[future]
	unsafe fn spawn(env: E, task: T, request: _) -> SpawnResult<Output> {
		#[cancel]
		fn cancel(context: NonNull<Context>) -> Result<()> {
			/* Safety: guaranteed by Future's contract */
			unsafe { Context::interrupt(context.as_pointer()) }
		}

		let mut spawn = Self { data: SpawnData::Uninit };

		/* Safety: worker_start never panics */
		let start = unsafe { Start::new(Self::worker_start, ptr!(&mut spawn).cast_const().cast()) };
		let executor = call_no_unwind(|| env.executor());

		/* Safety: guaranteed by caller */
		let worker = unsafe { ptr!(executor=>new_worker(start)) };

		spawn.data = SpawnData::Start(env, task, worker, request);

		let SpawnData::Start(_, _, worker, _) = &spawn.data else {
			/* Safety: we just assigned this */
			unsafe { unreachable_unchecked() };
		};

		/* Safety: the worker has not been started yet. the worker isn't exited until
		 * it completes */
		unsafe { ptr!(executor=>start(ptr!(worker))) };

		match replace(&mut spawn.data, SpawnData::Uninit) {
			SpawnData::Done(result) => Progress::Done(result),
			SpawnData::Pending(context, is_async) => {
				/* worker suspended without completing */

				/* Safety: pending task returns a valid pointer to a bool */
				unsafe { ptr!(*is_async) = true };

				Progress::Pending(cancel(context))
			}

			/* Safety: must be either pending or done */
			_ => unsafe { unreachable_unchecked() }
		}
	}
}

/// Spawn a new worker. The result of the worker will be returned as a
/// [`Future`]
///
/// # Safety
/// The `env` and `task` must outlive the spawned fiber
///
/// [`Future`]: crate::future::Future
#[future]
pub unsafe fn spawn_task<E, T, Output>(env: E, task: T, request: _) -> SpawnResult<Output>
where
	E: Environment,
	T: for<'ctx> Task<Output<'ctx> = Output>
{
	#[cancel]
	fn cancel(context: NonNull<Context>) -> Result<()> {
		/* use this fn to generate the cancel closure type */
		Ok(())
	}

	#[cfg(any(doc, feature = "xx-doc"))]
	unreachable!();

	#[cfg(not(any(doc, feature = "xx-doc")))]
	/* Safety: guaranteed by caller */
	(unsafe { SpawnWorker::spawn(env, task).run(request) })
}

/// Utility function that calls the above with the
/// executor and env passed to this function
///
/// # Safety
/// The cloned `env` and `task` must outlive the spawned fiber
#[future]
pub unsafe fn spawn_task_with_env<E, T, Output>(env: &E, task: T, request: _) -> SpawnResult<Output>
where
	E: Environment,
	T: for<'ctx> Task<Output<'ctx> = Output>
{
	#[cancel]
	fn cancel(context: NonNull<Context>) -> Result<()> {
		Ok(())
	}

	/* Safety: guaranteed by caller */
	unsafe { spawn_task(env.clone(), task).run(request) }
}

struct SpawnHandle<Output> {
	#[allow(clippy::type_complexity)]
	cancel: Option<CancelClosure<(NonNull<Context>, ReqPtr<SpawnResult<Output>>)>>,
	output: Option<SpawnResult<Output>>,
	waiter: ReqPtr<SpawnResult<Output>>
}

impl<Output> SpawnHandle<Output> {
	/// # Safety
	/// the future must be running
	unsafe fn try_cancel(&mut self) -> Option<Result<()>> {
		/* Safety: guaranteed by caller */
		self.cancel.take().map(|cancel| unsafe { cancel.run() })
	}
}

struct Spawn<Output> {
	request: Request<SpawnResult<Output>>,
	handle: UnsafeCell<SpawnHandle<Output>>
}

#[cfg(not(any(doc, feature = "xx-doc")))]
impl<Output> Spawn<Output> {
	/// # Safety
	/// must only be called when the task completes
	unsafe fn spawn_complete(
		_: ReqPtr<SpawnResult<Output>>, arg: Ptr<()>, output: SpawnResult<Output>
	) {
		/* Safety: we called into_raw if this task was in progress */
		let this = unsafe { Rc::from_raw(arg.cast::<Self>().as_ptr()) };

		/* Safety: exclusive unsafe cell access */
		let handle = unsafe { this.handle.as_mut() };

		if handle.waiter.is_null() {
			handle.output = Some(output);
		} else {
			/* Safety: complete the future */
			unsafe { Request::complete(handle.waiter, output) };
		}
	}

	fn new() -> Self {
		/* Safety: spawn_complete does not unwind */
		unsafe {
			/* request arg is assigned once pinned */
			Self {
				request: Request::new(Ptr::null(), Self::spawn_complete),
				handle: UnsafeCell::new(SpawnHandle {
					cancel: None,
					output: None,
					waiter: Ptr::null()
				})
			}
		}
	}

	/// # Safety
	/// The cloned `env` and `task` must outlive the spawned fiber
	unsafe fn run<E, T>(env: &E, task: T) -> JoinHandle<Output>
	where
		E: Environment,
		T: for<'ctx> Task<Output<'ctx> = Output>
	{
		/* Safety: we are never unpinned */
		let this = unsafe { Self::new().pin_rc().into_inner() };

		/* Safety: exclusive unsafe cell access */
		let handle = unsafe { this.handle.as_mut() };

		/* Safety: guaranteed by caller */
		match unsafe { spawn_task_with_env(env, task).run(ptr!(&this.request)) } {
			Progress::Done(result) => handle.output = Some(result),
			Progress::Pending(cancel) => {
				handle.cancel = Some(cancel);

				let _ = Rc::into_raw(this.clone());
			}
		}

		JoinHandle { task: this }
	}
}

impl<Output> Pin for Spawn<Output> {
	unsafe fn pin(&mut self) {
		let arg = ptr!(&*self).cast();

		self.request.set_arg(arg);
	}
}

/// A handle for joining with a [`fn@spawn`]ed async task
pub struct JoinHandle<Output> {
	task: Rc<Spawn<Output>>
}

#[asynchronous]
impl<Output> JoinHandle<Output> {
	/// # Safety
	/// caller must ensure an aliased &mut does not get created
	#[allow(clippy::mut_from_ref)]
	unsafe fn handle(&self) -> &mut SpawnHandle<Output> {
		/* Safety: guaranteed by caller */
		unsafe { self.task.handle.as_mut() }
	}

	/// # Safety
	/// future must be in progress
	unsafe fn try_cancel(&self) -> Result<()> {
		/* Safety: guaranteed by caller */
		unsafe { self.handle().try_cancel() }.unwrap_or(Ok(()))
	}

	#[future]
	fn join(self, request: _) -> SpawnResult<Output> {
		#[cancel]
		fn cancel(self) -> Result<()> {
			/* Safety: guaranteed by Future's contract. we may already be cancelling */
			unsafe { self.try_cancel() }
		}

		/* Safety: exclusive unsafe cell access */
		unsafe { self.handle().waiter = request };

		/* we could make JoinHandle return Progress::Done instead of checking below,
		 * but we want to avoid calling future::block_on if possible
		 */
		Progress::Pending(cancel(self))
	}

	#[must_use]
	pub fn is_done(&self) -> bool {
		/* Safety: exclusive unsafe cell access */
		unsafe { self.handle().output.is_some() }
	}

	/// Signals the task to cancel, without waiting for the result
	pub fn request_cancel(&self) -> Result<()> {
		if self.is_done() {
			Ok(())
		} else {
			/* Safety: task is running */
			unsafe { self.try_cancel() }
		}
	}

	pub async fn try_join(self) -> SpawnResult<Output> {
		/* Safety: exclusive unsafe cell access */
		if let Some(output) = unsafe { self.handle().output.take() } {
			/* task finished in-between spawn and await */
			output
		} else {
			block_on(self.join()).await
		}
	}

	/// Signals the task to cancel, waits for, and returns the result
	pub async fn cancel(self) -> Output {
		let result = self.request_cancel();

		if let Err(err) = &result {
			debug!(target: &self, ">> Cancel failed: {:?}", err);
		}

		self.await
	}
}

#[asynchronous(task)]
impl<Output> Task for JoinHandle<Output> {
	type Output = Output;

	async fn run(self) -> Output {
		let result = self.try_join().await;

		runtime::join(result)
	}
}

/// Spawn a new async task
///
/// Returns a [`JoinHandle`] which may be used to get the result from the task
///
/// # Safety
/// The cloned `env` and `task` must outlive the spawned fiber
pub unsafe fn spawn<E, T, Output>(env: &E, task: T) -> JoinHandle<Output>
where
	E: Environment,
	T: for<'ctx> Task<Output<'ctx> = Output>
{
	#[cfg(any(doc, feature = "xx-doc"))]
	unreachable!();

	#[cfg(not(any(doc, feature = "xx-doc")))]
	/* Safety: guaranteed by caller */
	(unsafe { Spawn::run(env, task) })
}
