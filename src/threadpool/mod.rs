use std::mem::transmute;
use std::os::unix::thread::JoinHandleExt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::cell::UnsafeCell;
use crate::closure::DynFnOnce;
use crate::container::intrusive::linked_list::*;
use crate::error::*;
use crate::future::*;
use crate::os::signal::*;
use crate::os::unistd::{get_system_configuration, SystemConfiguration};
use crate::pointer::*;
use crate::runtime::call_no_unwind;
use crate::{debug, error, trace, warn};

pub struct TaskContext {
	cancelled: AtomicBool
}

impl TaskContext {
	const fn new() -> Self {
		Self { cancelled: AtomicBool::new(false) }
	}

	pub fn cancelled(&self) -> bool {
		self.cancelled.load(Ordering::Relaxed)
	}
}

pub struct Work<'a> {
	node: Node,
	callback: DynFnOnce<'a, &'static TaskContext, ()>,
	request: ReqPtr<bool>,
	worker: Ptr<Worker>
}

impl<'a> Work<'a> {
	/// # Safety
	/// the lifetime for the `TaskContext` is not actually static
	///
	/// the caller must ensure the function is not dropped twice
	/// by checking the return value
	///
	/// the function must never unwind and must be safe to send to another
	/// thread
	#[must_use]
	pub const unsafe fn new(callback: DynFnOnce<'a, &'static TaskContext, ()>) -> Self {
		Self {
			node: Node::new(),
			callback,
			request: ReqPtr::null(),
			worker: Ptr::null()
		}
	}
}

/// # Safety
/// `list` contains all jobs that have yet to be started
unsafe fn cancel_all(list: &mut LinkedList) {
	/* Safety: list is pinned */
	while let Some(node) = unsafe { list.pop_front() } {
		/* Safety: all nodes are wrapped in Work */
		let work = unsafe { container_of!(node, Work<'_> =>node) };

		/* Safety: guaranteed by Future's contract */
		let request = unsafe { ptr!(work=>request) };

		/* Safety: complete the future */
		unsafe { Request::complete(request, false) };
	}
}

struct Queue {
	idle_count: AtomicUsize,
	work: Mutex<LinkedList>,
	notify: Condvar,
	closed: AtomicBool
}

impl Queue {
	const fn new(threads: usize) -> Self {
		Self {
			idle_count: AtomicUsize::new(threads),
			work: Mutex::new(LinkedList::new()),
			notify: Condvar::new(),
			closed: AtomicBool::new(false)
		}
	}
}

impl Drop for Queue {
	fn drop(&mut self) {
		#[allow(clippy::unwrap_used)]
		let work_queue = self.work.get_mut().unwrap();

		/* Safety: this is a work queue */
		unsafe { cancel_all(work_queue) };
	}
}

impl Pin for Queue {
	#[allow(clippy::unwrap_used)]
	unsafe fn pin(&mut self) {
		/* Safety: guaranteed by caller */
		unsafe { self.work.get_mut().unwrap().pin() }
	}
}

/* Safety: internal use only */
unsafe impl Send for Queue {}

/* Safety: internal use only */
unsafe impl Sync for Queue {}

struct Worker {
	thread: AtomicU64,
	queue: Pinned<Arc<Queue>>,
	cur_work: UnsafeCell<Ptr<Work<'static>>>,
	context: TaskContext
}

/* Safety: internal use only */
unsafe impl Send for Worker {}

/* Safety: internal use only */
unsafe impl Sync for Worker {}

impl Worker {
	const fn new(queue: Pinned<Arc<Queue>>) -> Self {
		Self {
			thread: AtomicU64::new(0),
			queue,
			cur_work: UnsafeCell::new(Ptr::null()),
			context: TaskContext::new()
		}
	}

	#[allow(
		clippy::unwrap_used,
		clippy::multiple_unsafe_ops_per_block,
		clippy::missing_panics_doc
	)]
	fn run(&self) {
		let mut work_queue = self.queue.work.lock().unwrap();

		self.queue.idle_count.fetch_sub(1, Ordering::Relaxed);

		while !self.queue.closed.load(Ordering::Relaxed) {
			/* Safety: list is pinned */
			let Some(node) = (unsafe { work_queue.pop_front() }) else {
				self.queue.idle_count.fetch_add(1, Ordering::Relaxed);

				work_queue = self.queue.notify.wait(work_queue).unwrap();

				self.queue.idle_count.fetch_sub(1, Ordering::Relaxed);

				continue;
			};

			/* Safety: all nodes are wrapped in Work */
			let work_ptr = unsafe { container_of!(node, Work<'_> =>node) }.cast_mut();

			/* Safety: guaranteed by Future's contract */
			let work = unsafe { work_ptr.as_mut() };
			let (callback, request) = (&work.callback, work.request);

			work.worker = ptr!(self);

			/* Safety: lock held */
			unsafe { ptr!(*self.cur_work) = work_ptr.cast().cast_const() };

			self.context.cancelled.store(false, Ordering::Relaxed);

			drop(work_queue);

			/* Safety: guaranteed by caller */
			call_no_unwind(|| unsafe { callback.call_once_ref(transmute(&self.context)) });

			/* Safety: send the completion */
			unsafe { Request::complete(request, true) };

			work_queue = self.queue.work.lock().unwrap();

			/* Safety: lock held */
			unsafe { ptr!(*self.cur_work) = Ptr::null() };
		}
	}
}

#[allow(missing_copy_implementations)]
pub struct CancelWork(MutPtr<Work<'static>>);

#[allow(clippy::cast_possible_wrap)]
const INTERRUPT_SIGNAL: i32 = (SIGRTMIN + Signal::Interrupt as u32) as i32;

pub struct ThreadPool {
	workers: Box<[Arc<Worker>]>,
	queue: Pinned<Arc<Queue>>,
	interruptible: bool
}

impl ThreadPool {
	fn install_interrupt_handler() -> OsResult<()> {
		const extern "C" fn handler(_: i32) {}

		let mut action = SigAction::default();

		action.handler.handler = Some(handler);

		sig_action(INTERRUPT_SIGNAL, Some(&action), None)
	}

	fn interrupt_worker(&self, worker: &Worker) {
		if !self.interruptible {
			return;
		}

		let thread = worker.thread.load(Ordering::Relaxed);

		if let Err(err) = pthread_signal(thread, INTERRUPT_SIGNAL) {
			warn!(target: self, "== Failed to interrupt worker {:?}", err);
		}
	}

	#[allow(clippy::missing_panics_doc, clippy::expect_used)]
	pub fn new(max_workers: usize) -> Result<Self> {
		let queue = Queue::new(max_workers).pin_arc();
		let mut threads = Vec::with_capacity(max_workers);
		let mut error = None;

		for i in 0..max_workers {
			let worker = Arc::new(Worker::new(queue.clone()));
			let worker_clone = worker.clone();

			let result = thread::Builder::new()
				.name(format!("xx-tp-wrk-{}", i))
				.spawn(move || worker.run());

			let handle = match result {
				Ok(handle) => handle,
				Err(err) => {
					error = Some(err);

					break;
				}
			};

			worker_clone
				.thread
				.store(handle.as_pthread_t(), Ordering::Relaxed);
			threads.push(worker_clone);
		}

		let mut this = Self {
			workers: threads.into_boxed_slice(),
			queue,
			interruptible: true
		};

		if let Err(err) = Self::install_interrupt_handler() {
			warn!(
				target: &this,
				"== Failed to set interrupt handler: {:?}\n== Cancel requests may not be possible",
				err
			);

			this.interruptible = false;
		}

		if let Some(err) = error {
			error!(target: &this, "== Failed to create thread pool {:?}", err);

			this.close();

			return Err(err.into());
		}

		debug!(target: &this, "++ Created thread pool with {} workers", max_workers);

		Ok(this)
	}

	#[allow(clippy::expect_used, clippy::missing_panics_doc)]
	pub fn new_with_default_count() -> Result<Self> {
		let count = get_system_configuration(SystemConfiguration::NprocessorsOnln)?
			.expect("Falied to get cpu count");
		let mut count = count.try_into().unwrap_or(usize::MAX);

		count = count.checked_mul(2).unwrap_or(usize::MAX);

		Self::new(count)
	}

	/// # Safety
	/// See [`Future::run`]
	#[allow(
		clippy::unwrap_used,
		clippy::multiple_unsafe_ops_per_block,
		clippy::missing_panics_doc,
		clippy::must_use_candidate
	)]
	pub unsafe fn submit_direct(
		&self, work: MutPtr<Work<'_>>, request: ReqPtr<bool>
	) -> CancelWork {
		let work_queue = self.queue.work.lock().unwrap();

		/* Safety: exclusive access here */
		unsafe {
			ptr!(work=>request) = request;

			work_queue.append(ptr!(&mut work=>node).cast_const());
		}

		let notify = self.queue.idle_count.load(Ordering::Relaxed) != 0;

		if notify {
			self.queue.notify.notify_one();
		}

		trace!(target: self, "## submit_direct(work = {:?}): notified = {}", work, notify);

		CancelWork(work.cast())
	}

	/// # Safety
	/// See [`Cancel::run`]
	#[allow(
		clippy::unwrap_used,
		clippy::multiple_unsafe_ops_per_block,
		clippy::missing_panics_doc,
		clippy::needless_pass_by_value
	)]
	pub unsafe fn cancel_direct(&self, cancel: CancelWork) {
		let work_queue = self.queue.work.lock().unwrap();

		/* Safety: mutable access is tied to the mutex */
		let work = unsafe { cancel.0.as_mut() };

		if work.node.linked() {
			trace!(target: self, "## cancel_direct(work = {:?}) = SyncCancel", cancel.0);

			/* Safety: the node is linked */
			unsafe { work.node.unlink_unchecked() };

			/* Safety: the work was removed from the list. no further completions are
			 * possible */
			unsafe { Request::complete(work.request, false) };
		} else {
			/* Safety: workers must be valid */
			let worker = unsafe { work.worker.as_ref() };

			/* Safety: lock held */
			let cur_work = unsafe { ptr!(*worker.cur_work) };

			if cancel.0 == cur_work.cast_mut() {
				worker.context.cancelled.store(true, Ordering::Relaxed);

				drop(work_queue);

				trace!(target: self, "## cancel_direct(work = {:?}) = AsyncCancel(interrupted = {})", cancel.0, self.interruptible);

				self.interrupt_worker(worker);
			} else {
				trace!(target: self, "## cancel_direct(work = {:?}) = AlreadyCompleted", cancel.0);
			}
		}
	}

	#[future]
	pub unsafe fn submit(&self, work: MutPtr<Work<'_>>, request: _) -> bool {
		#[cancel]
		fn cancel(&self, cancel: CancelWork) -> Result<()> {
			self.cancel_direct(cancel);

			Ok(())
		}

		/* Safety: caller must uphold Future's contract */
		let token = unsafe { self.submit_direct(work, request) };

		Progress::Pending(cancel(self, token))
	}

	#[allow(clippy::missing_panics_doc, clippy::unwrap_used)]
	pub fn cancel_all(&self) {
		let mut work_queue = self.queue.work.lock().unwrap();

		for worker in &self.workers {
			self.interrupt_worker(worker);
		}

		/* Safety: this is a work queue */
		unsafe { cancel_all(&mut work_queue) };

		self.queue.notify.notify_all();
	}

	pub fn close(&self) {
		self.queue.closed.store(true, Ordering::Relaxed);
		self.cancel_all();
	}
}

impl Drop for ThreadPool {
	fn drop(&mut self) {
		self.close();
	}
}
