#![allow(unreachable_pub, dead_code)]

use std::marker::PhantomData;
use std::sync::atomic::Ordering::*;

use super::*;
use crate::cell::Cell;
use crate::container::intrusive::linked_list::*;
use crate::coroutines::{self, block_on};
use crate::future::*;
use crate::runtime::{catch_unwind_safe, join, MaybePanic};
use crate::sync::atomic::AtomicPtr;
use crate::sync::{SpinMutex, SpinMutexGuard};

#[errors]
pub enum WaitError {
	#[display("Suspend cancelled")]
	#[kind = ErrorKind::Interrupted]
	Cancelled,

	#[display("Wait list closed")]
	Closed
}

type WaitResult<T> = result::Result<T, WaitError>;

#[asynchronous]
async fn check_interrupt() -> WaitResult<()> {
	coroutines::check_interrupt()
		.await
		.map_err(|_| WaitError::Cancelled)
}

const fn closed<T>() -> ReqPtr<T> {
	Ptr::from_addr(usize::MAX)
}

pub struct AtomicWaiter<T = ()> {
	waiter: AtomicPtr<Request<WaitResult<T>>>
}

#[asynchronous]
impl<T> AtomicWaiter<T> {
	#[must_use]
	pub const fn new() -> Self {
		Self { waiter: AtomicPtr::new(ReqPtr::null()) }
	}

	fn set_waiter(&self, request: ReqPtr<WaitResult<T>>) -> ReqPtr<WaitResult<T>> {
		let result = self.waiter.fetch_update(SeqCst, Relaxed, |prev| {
			(prev != closed()).then_some(request)
		});

		result.unwrap_or_else(|err| err)
	}

	fn cancel_wait(
		&self, request: ReqPtr<WaitResult<T>>
	) -> result::Result<ReqPtr<WaitResult<T>>, ReqPtr<WaitResult<T>>> {
		self.waiter
			.compare_exchange(request, Ptr::null(), Relaxed, Relaxed)
	}

	#[future]
	unsafe fn suspend<F>(&self, should_block: F, request: _) -> WaitResult<T>
	where
		F: FnOnce() -> bool
	{
		#[cancel]
		fn cancel(&self) -> Result<()> {
			/* wake may already be in progress */
			let result = self.cancel_wait(request);

			if result.is_ok() {
				/* Safety: we took ownership of waking up the task. send the cancellation */
				unsafe { Request::complete(request, Err(WaitError::Cancelled)) };
			}

			Ok(())
		}

		let mut error = None;
		let prev = self.set_waiter(request);

		if !should_block() {
			/* wake may already be in progress */
			if self.cancel_wait(request).is_ok() {
				error = Some(WaitError::Cancelled);
			}
		}

		if !prev.is_null() {
			if prev != closed() {
				/* Safety: we took ownership of waking up the task. send the cancellation */
				unsafe { Request::complete(prev, Err(WaitError::Cancelled)) };
			} else {
				error = Some(WaitError::Closed);
			}
		}

		match error {
			None => Progress::Pending(cancel(self)),
			Some(err) => Progress::Done(Err(err))
		}
	}

	pub async fn wait(&self) -> WaitResult<T> {
		check_interrupt().await?;

		/* Safety: callback doesn't unwind */
		block_on(unsafe { self.suspend(|| true) }).await
	}

	pub async fn wait_thread_safe(&self) -> WaitResult<T> {
		check_interrupt().await?;

		/* Safety: callback doesn't unwind */
		block_on_thread_safe(unsafe { self.suspend(|| true) }).await
	}

	/// # Safety
	/// `should_block` must never unwind
	pub async unsafe fn wait_thread_safe_check<F>(&self, should_block: F) -> WaitResult<T>
	where
		F: FnOnce() -> bool
	{
		check_interrupt().await?;

		/* Safety: guaranteed by caller */
		block_on_thread_safe(unsafe { self.suspend(should_block) }).await
	}

	/// # Safety
	/// `should_block` and `should_cancel` must never unwind
	pub unsafe fn blocking_wait<F, C>(&self, should_block: F, should_cancel: C) -> WaitResult<T>
	where
		F: FnOnce() -> bool,
		C: Fn() -> bool
	{
		/* Safety: guaranteed by caller */
		let suspend = unsafe { self.suspend(should_block) };

		/* Safety: guaranteed by caller */
		unsafe { block_on_sync(suspend, should_cancel) }
	}

	fn wake_internal(&self, new_waiter: ReqPtr<WaitResult<T>>, value: T) -> bool {
		let prev = self.set_waiter(new_waiter);

		if prev.is_null() || prev == closed() {
			return false;
		}

		/* Safety: complete the future */
		unsafe { Request::complete(prev, Ok(value)) };

		true
	}

	pub fn wake(&self, value: T) -> bool {
		self.wake_internal(Ptr::null(), value)
	}

	pub fn close(&self, value: T) -> bool {
		self.wake_internal(closed(), value)
	}

	pub fn is_closed(&self) -> bool {
		self.waiter.load(SeqCst) == closed()
	}
}

struct Waiter<T> {
	node: Node,
	request: ReqPtr<WaitResult<MaybePanic<T>>>
}

impl<T> Waiter<T> {
	/// # Safety
	/// valid `this` ptr
	/// must no longer be linked
	/// must be called no more than once
	unsafe fn complete(this: Ptr<Self>, value: WaitResult<MaybePanic<T>>) {
		/* Safety: guaranteed by caller */
		let request = unsafe { ptr!(this=>request) };

		/* Safety: complete the future */
		unsafe { Request::complete(request, value) };
	}

	/// # Safety
	/// valid `node` ptr
	/// must no longer be linked
	/// must be called no more than once
	unsafe fn complete_node(node: Ptr<Node>, value: WaitResult<MaybePanic<T>>) {
		/* Safety: guaranteed by caller */
		let waiter = unsafe { container_of!(node, Self=>node) };

		/* Safety: guaranteed by caller */
		unsafe { Self::complete(waiter, value) }
	}
}

pub struct RawWaitList<T = ()> {
	list: LinkedList,
	count: Cell<usize>,
	closed: Cell<bool>,
	phantom: PhantomData<T>
}

#[asynchronous]
impl<T: Clone> RawWaitList<T> {
	/// # Safety
	/// must pin the list before calling any of its methods
	#[must_use]
	pub const unsafe fn new() -> Self {
		Self {
			list: LinkedList::new(),
			count: Cell::new(0),
			closed: Cell::new(false),
			phantom: PhantomData
		}
	}

	/// # Safety
	/// Waiter must be pinned, unlinked, and live until it's waked
	#[future]
	unsafe fn suspend(&self, waiter: &mut Waiter<T>, request: _) -> WaitResult<MaybePanic<T>> {
		#[cancel]
		fn cancel(waiter: MutPtr<Waiter<T>>) -> Result<()> {
			/* Safety: we linked this node earlier */
			#[allow(clippy::multiple_unsafe_ops_per_block)]
			(unsafe { ptr!(waiter=>node.unlink_unchecked()) });

			/* Safety: send the cancellation. waiter is unlinked, so there won't be
			 * another completion
			 *
			 * note: the waiter may no longer be valid after this call
			 */
			unsafe { Waiter::complete(waiter.cast_const(), Err(WaitError::Cancelled)) };

			Ok(())
		}

		waiter.request = request;

		/* Safety: guaranteed by caller
		 *
		 * note: even though we have &mut to the Waiter, and &mut Cell<T> by
		 * extension, it's not actually UB if the node's pointers get changed
		 * as long as we don't call `Cell::get_mut`
		 */
		unsafe { self.list.append(ptr!(&waiter.node)) };

		Progress::Pending(cancel(ptr!(waiter)))
	}

	pub async fn wait(&self) -> WaitResult<T> {
		if self.closed.get() {
			return Err(WaitError::Closed);
		}

		check_interrupt().await?;

		/* we don't really care if it overflows */
		#[allow(clippy::arithmetic_side_effects)]
		self.count.update(|count| count + 1);

		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		/* Safety: waiter is new, pinned, and lives until it completes */
		let result = block_on(unsafe { self.suspend(&mut waiter) }).await;

		#[allow(clippy::arithmetic_side_effects)]
		self.count.update(|count| count - 1);

		result.map(join)
	}

	/// # Safety
	/// `should_cancel` must never unwind
	pub unsafe fn blocking_wait<C>(&self, should_cancel: C) -> WaitResult<T>
	where
		C: Fn() -> bool
	{
		if self.closed.get() {
			return Err(WaitError::Closed);
		}

		/* we don't really care if it overflows */
		#[allow(clippy::arithmetic_side_effects)]
		self.count.update(|count| count + 1);

		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		/* Safety: guaranteed by caller */
		let suspend = unsafe { self.suspend(&mut waiter) };

		/* Safety: guaranteed by caller */
		let result = unsafe { block_on_sync(suspend, should_cancel) };

		#[allow(clippy::arithmetic_side_effects)]
		self.count.update(|count| count - 1);

		result.map(join)
	}

	pub fn wake_one(&self, value: T) -> bool {
		/* Safety: list is pinned */
		let Some(node) = (unsafe { self.list.pop_front() }) else {
			return false;
		};

		/* Safety: complete the future */
		unsafe { Waiter::complete_node(node, Ok(Ok(value))) };

		true
	}

	pub fn wake_all(&self, value: T) -> usize {
		let list = LinkedList::new();
		let count = self.count.get();

		pin!(list);

		/* Safety: our new list is pinned, and we clear out all nodes before
		 * returning
		 */
		unsafe { self.list.move_elements(&list) };

		/* Safety: list is pinned */
		while let Some(node) = unsafe { list.pop_front() } {
			/*
			 * Safety: complete the future
			 */
			#[allow(clippy::multiple_unsafe_ops_per_block)]
			unsafe {
				if list.is_empty() {
					Waiter::complete_node(node, Ok(Ok(value)));

					break;
				}

				Waiter::complete_node(node, Ok(catch_unwind_safe(|| value.clone())));
			};
		}

		count
	}

	pub fn close(&self, value: T) -> usize {
		self.closed.set(true);
		self.wake_all(value)
	}
}

impl<T> Pin for RawWaitList<T> {
	unsafe fn pin(&mut self) {
		/* Safety: we are being pinned */
		unsafe { self.list.pin() };
	}
}

type PinBoxList = Pinned<Box<LinkedList>>;

pub struct ThreadSafeWaitList<T = ()> {
	list: SpinMutex<PinBoxList>,
	count: AtomicUsize,
	closed: AtomicBool,
	empty: AtomicBool,
	phantom: PhantomData<T>
}

struct Counter<'a>(&'a AtomicUsize);

impl<'a> Counter<'a> {
	fn new(count: &'a AtomicUsize) -> Self {
		count.fetch_add(1, Relaxed);

		Self(count)
	}
}

impl Drop for Counter<'_> {
	fn drop(&mut self) {
		self.0.fetch_sub(1, Relaxed);
	}
}

struct Empty<'a> {
	list: &'a SpinMutexGuard<'a, PinBoxList>,
	empty: &'a AtomicBool
}

impl<'a> Empty<'a> {
	fn new(list: &'a SpinMutexGuard<'a, PinBoxList>, empty: &'a AtomicBool) -> Self {
		empty.store(false, SeqCst);

		Self { list, empty }
	}
}

impl Drop for Empty<'_> {
	fn drop(&mut self) {
		self.empty.store(self.list.is_empty(), Relaxed);
	}
}

#[asynchronous]
impl<T: Clone> ThreadSafeWaitList<T> {
	#[must_use]
	pub fn new() -> Self {
		Self {
			list: SpinMutex::new(LinkedList::new().pin_box()),
			count: AtomicUsize::new(0),
			closed: AtomicBool::new(false),
			empty: AtomicBool::new(false),
			phantom: PhantomData
		}
	}

	/// # Safety
	/// Waiter must be pinned, unlinked, and live until it's waked
	#[future]
	unsafe fn suspend<F>(
		&self, waiter: &mut Waiter<T>, should_block: F, request: _
	) -> WaitResult<MaybePanic<T>>
	where
		F: FnOnce() -> bool
	{
		#[cancel]
		fn cancel(&self, waiter: MutPtr<Waiter<T>>) -> Result<()> {
			#[allow(clippy::unwrap_used)]
			let list = self.list.lock();

			/* Safety: guaranteed by future's contract */
			let waiter = unsafe { waiter.as_ref() };

			/* the node may have already been unlinked */
			if !waiter.node.linked() {
				return Ok(());
			}

			/* Safety: the node hasn't been unlinked yet */
			unsafe { waiter.node.unlink() };

			drop(list);

			/* Safety: send the cancellation. waiter is unlinked, so there won't be
			 * another completion
			 *
			 * note: the waiter may no longer be valid after this call
			 */
			unsafe { Request::complete(waiter.request, Err(WaitError::Cancelled)) };

			Ok(())
		}

		waiter.request = request;

		let list = self.list.lock();

		if self.is_closed() {
			return Progress::Done(Err(WaitError::Closed));
		}

		let empty = Empty::new(&list, &self.empty);

		if !should_block() {
			return Progress::Done(Err(WaitError::Cancelled));
		}

		forget(empty);

		/* Safety: guaranteed by caller */
		unsafe { list.append(ptr!(&waiter.node)) };

		Progress::Pending(cancel(self, ptr!(waiter)))
	}

	pub async fn wait<F>(&self, should_block: F) -> WaitResult<T>
	where
		F: FnOnce() -> bool
	{
		check_interrupt().await?;

		let counter = Counter::new(&self.count);
		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		/* Safety: waiter is new, pinned, and lives until it completes */
		let result = block_on_thread_safe(unsafe { self.suspend(&mut waiter, should_block) }).await;

		drop(counter);

		result.map(join)
	}

	/// # Safety
	/// `should_cancel` must never unwind
	pub unsafe fn blocking_wait<F, C>(&self, should_block: F, should_cancel: C) -> WaitResult<T>
	where
		F: FnOnce() -> bool,
		C: Fn() -> bool
	{
		let counter = Counter::new(&self.count);
		let mut waiter = Waiter { node: Node::new(), request: Ptr::null() };

		/* Safety: guaranteed by caller */
		let suspend = unsafe { self.suspend(&mut waiter, should_block) };

		/* Safety: guaranteed by caller */
		let result = unsafe { block_on_sync(suspend, should_cancel) };

		drop(counter);

		result.map(join)
	}

	pub fn wake_one(&self, value: T) -> bool {
		if self.empty.load(SeqCst) {
			return false;
		}

		/* Safety: list is pinned */
		let Some(node) = (unsafe { self.list.lock().pop_front() }) else {
			return false;
		};

		/* Safety: complete the future */
		unsafe { Waiter::complete_node(node, Ok(Ok(value))) };

		true
	}

	pub fn wake_all(&self, value: T) -> usize {
		if self.empty.load(SeqCst) {
			return 0;
		}

		let count = self.count.load(Relaxed);
		let list = self.list.lock();

		/* Safety: list is pinned */
		while let Some(node) = unsafe { list.pop_front() } {
			/*
			 * Safety: complete the future
			 */
			#[allow(clippy::multiple_unsafe_ops_per_block)]
			unsafe {
				if list.is_empty() {
					Waiter::complete_node(node, Ok(Ok(value)));

					break;
				}

				Waiter::complete_node(node, Ok(catch_unwind_safe(|| value.clone())));
			};
		}

		count
	}

	pub fn is_closed(&self) -> bool {
		self.closed.load(Relaxed)
	}

	pub fn close(&self, value: T) -> usize {
		self.closed.store(true, Relaxed);
		self.wake_all(value)
	}
}
