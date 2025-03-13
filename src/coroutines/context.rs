#![allow(clippy::multiple_unsafe_ops_per_block)]

use std::any::TypeId;
use std::hash::{DefaultHasher, Hash, Hasher};

use super::*;
use crate::cell::*;
use crate::closure::*;
use crate::impls::OptionExt;
use crate::macros::const_assert;

fn type_for<E>() -> u32
where
	E: 'static
{
	let mut hasher = DefaultHasher::new();

	TypeId::of::<E>().hash(&mut hasher);

	/* comparing `TypeId`s is generally slower than u32
	 *
	 * u32 is enough to ensure that two different environments
	 * are in fact different
	 */
	#[allow(clippy::cast_possible_truncation)]
	(hasher.finish() as u32)
}

/// # Safety
/// the worker must belong to this thread
unsafe fn wake(_: ReqPtr<()>, arg: Ptr<()>, _: ()) {
	let worker = arg.cast::<Worker>();

	/* Safety: guaranteed by caller */
	unsafe { ptr!(worker=>resume()) };
}

type Canceller = DynFnOnce<'static, (), Result<()>>;

struct Data {
	budget: Cell<u16>,
	guards: Cell<u32>,
	interrupted: Cell<bool>,
	cancel: UnsafeCell<Option<Canceller>>
}

impl Data {
	const fn new() -> Self {
		const_assert!(DEFAULT_BUDGET <= u16::MAX as u32);

		Self {
			#[allow(clippy::cast_possible_truncation)]
			budget: Cell::new(DEFAULT_BUDGET as u16),
			guards: Cell::new(0),
			interrupted: Cell::new(false),
			cancel: UnsafeCell::new(None)
		}
	}
}

#[cfg_attr(not(any(doc, feature = "xx-doc")), repr(C))]
pub struct Context {
	environment: u32,
	worker: Ptr<Worker>,
	waker: Option<Waker>,
	data: Data
}

impl Context {
	/// Runs async task `T`
	///
	/// # Safety
	/// See [`scoped`]
	#[cfg(not(any(doc, feature = "xx-doc")))]
	#[inline(always)]
	pub(super) unsafe fn run<T>(&self, task: T) -> T::Output<'_>
	where
		T: Task
	{
		/* Safety: guaranteed by caller */
		unsafe { task.run(self) }
	}

	/// # Safety
	/// same as Worker::suspend
	pub(super) unsafe fn suspend(&self) {
		/* Safety: guaranteed by caller */
		unsafe { ptr!(self.worker=>suspend()) };
	}

	/// # Safety
	/// same as Worker::resume
	pub(super) unsafe fn resume(&self) {
		/* Safety: guaranteed by caller */
		unsafe { ptr!(self.worker=>resume()) };
	}

	/// # Safety
	/// See [`scoped`]
	///
	/// # Panics
	/// See [`block_on_thread_safe`]
	#[inline]
	pub(super) unsafe fn block_on<F>(&self, future: F, thread_safe: bool) -> F::Output
	where
		F: Future
	{
		let block = move |cancel: F::Cancel| {
			/* Safety: the cancel is set to None when the future completes */
			let mut canceller = FnCallOnce::new(move |()| unsafe { cancel.run() });

			/* Safety: exclusive access */
			unsafe { ptr!(*self.data.cancel) = Some(canceller.as_ptr()) };

			if thread_safe {
				/* Safety: waker is Some as checked below */
				let waker = unsafe { self.waker.as_ref().unwrap_unchecked() };

				/* Safety: valid ptr */
				unsafe { waker.prepare() };
			}

			#[allow(clippy::cast_possible_truncation)]
			self.data.budget.set(DEFAULT_BUDGET as u16);

			/* Safety: context is valid while executing */
			unsafe {
				self.suspend();

				ptr!(*self.data.cancel) = None;
			}
		};

		if thread_safe {
			#[allow(clippy::expect_used)]
			let waker = self.waker.as_ref().expect("Operation not supported");

			/* Safety: wake doesn't unwind */
			let request = unsafe { Request::new(self.worker.cast(), wake) };
			let req_ptr = ptr!(&request);

			/* Safety: context is valid while executing */
			let resume = move || unsafe { waker.wake(req_ptr) };

			/* Safety: we are blocked until the future completes */
			unsafe { future::block_on(block, resume, future) }
		} else {
			let worker = self.worker;

			/* Safety: context is valid while executing */
			let resume = move || unsafe { ptr!(worker=>resume()) };

			/* Safety: we are blocked until the future completes */
			unsafe { future::block_on(block, resume, future) }
		}
	}

	pub(super) fn current_budget(&self) -> u16 {
		self.data.budget.get()
	}

	pub(super) fn decrease_budget(&self, amount: u16) -> Option<u16> {
		let result = self.data.budget.get().checked_sub(amount);

		self.data.budget.set(result.unwrap_or(0));

		result
	}

	/// Returns true if the worker is being interrupted
	///
	/// In the presence of interrupt guards, this returns false
	pub(super) fn interrupted(&self) -> bool {
		self.data.guards == 0 && self.data.interrupted.get()
	}

	/// Clears any interrupts or pending interrupts (due to guards) on the
	/// current worker
	pub(super) fn clear_interrupt(&self) {
		self.data.interrupted.set(false);
	}

	/// # Safety
	/// the context must be alive while it's executing
	/// this function is unsafe so that Context::run doesn't need
	/// these guarantees
	///
	/// must call `set_worker`
	#[must_use]
	pub unsafe fn new<E>(waker: Option<Waker>) -> Self
	where
		E: Environment
	{
		Self {
			environment: type_for::<E>(),
			worker: Ptr::null(),
			waker,
			data: Data::new()
		}
	}

	/// # Safety
	/// worker must be a valid pointer, and must outlive this context
	pub unsafe fn set_worker(&mut self, worker: Ptr<Worker>) {
		self.worker = worker;
	}

	/// Signals an interrupt to the current task
	///
	/// # Safety
	/// See `Cancel::run`
	pub unsafe fn interrupt(this: Ptr<Self>) -> Result<()> {
		/* Safety: guaranteed by caller */
		let this = unsafe { this.as_ref() };
		let interrupted = this.data.interrupted.replace(true);

		#[allow(clippy::never_loop)]
		loop {
			if this.data.guards > 0 {
				break;
			}

			/* Safety: exclusive access */
			let Some(canceller) = (unsafe { ptr!(this.data.cancel=>take()) }) else {
				break;
			};

			/* this function may recursively call itself if a task
			 * is awaiting itself
			 *
			 * the context may no longer be valid after this call
			 */
			return canceller.call_once(());
		}

		if !interrupted {
			Ok(())
		} else {
			Err(ErrorKind::AlreadyInProgress.into())
		}
	}

	pub fn get_environment<E>(&self) -> Option<&E>
	where
		E: Environment
	{
		if self.environment == type_for::<E>() {
			/* Safety: type is checked */
			Some(call_no_unwind(|| unsafe { E::from_context(self) }))
		} else {
			None
		}
	}
}

pub struct InterruptGuard<'ctx> {
	context: &'ctx Context
}

impl<'ctx> InterruptGuard<'ctx> {
	fn update_guard_count(&self, rel: i32) {
		self.context.data.guards.update(|guards| {
			guards
				.checked_add_signed(rel)
				/* this can never happen unless there is UB. useful to check anyway as
				 * it doesn't have to be fast
				 */
				.expect_nounwind("Interrupt guards count overflowed")
		});
	}

	pub(super) fn new(context: &'ctx Context) -> Self {
		let this = Self { context };

		this.update_guard_count(1);
		this
	}
}

impl Drop for InterruptGuard<'_> {
	fn drop(&mut self) {
		self.update_guard_count(-1);
	}
}
