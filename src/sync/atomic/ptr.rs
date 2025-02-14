use std::mem::transmute;
use std::sync::atomic;

use crate::pointer::*;

pub struct AtomicPointer<T, const MUT: bool> {
	ptr: atomic::AtomicPtr<T>
}

pub type AtomicMutPtr<T> = AtomicPointer<T, true>;
pub type AtomicPtr<T> = AtomicPointer<T, false>;

impl<T, const MUT: bool> AtomicPointer<T, MUT> {
	#[must_use]
	pub const fn new(value: Pointer<T, MUT>) -> Self {
		Self { ptr: atomic::AtomicPtr::new(value.ptr()) }
	}

	pub fn get_mut(&mut self) -> &mut Pointer<T, MUT> {
		/* Safety: repr transparent */
		#[allow(clippy::transmute_ptr_to_ptr)]
		unsafe {
			transmute(self.ptr.get_mut())
		}
	}

	pub const fn into_inner(self) -> Pointer<T, MUT> {
		Pointer { ptr: self.ptr.into_inner() }
	}

	pub fn load(&self, order: atomic::Ordering) -> Pointer<T, MUT> {
		Pointer { ptr: self.ptr.load(order) }
	}

	pub fn store(&self, value: Pointer<T, MUT>, order: atomic::Ordering) {
		self.ptr.store(value.ptr(), order);
	}

	pub fn swap(&self, value: Pointer<T, MUT>, order: atomic::Ordering) -> Pointer<T, MUT> {
		Pointer { ptr: self.ptr.swap(value.ptr(), order) }
	}

	pub fn compare_exchange(
		&self, current: Pointer<T, MUT>, new: Pointer<T, MUT>, success: atomic::Ordering,
		failure: atomic::Ordering
	) -> Result<Pointer<T, MUT>, Pointer<T, MUT>> {
		match self
			.ptr
			.compare_exchange(current.ptr(), new.ptr(), success, failure)
		{
			Ok(ptr) => Ok(Pointer { ptr }),
			Err(ptr) => Err(Pointer { ptr })
		}
	}

	pub fn compare_exchange_weak(
		&self, current: Pointer<T, MUT>, new: Pointer<T, MUT>, success: atomic::Ordering,
		failure: atomic::Ordering
	) -> Result<Pointer<T, MUT>, Pointer<T, MUT>> {
		match self
			.ptr
			.compare_exchange_weak(current.ptr(), new.ptr(), success, failure)
		{
			Ok(ptr) => Ok(Pointer { ptr }),
			Err(ptr) => Err(Pointer { ptr })
		}
	}

	pub fn fetch_update<F>(
		&self, set_order: atomic::Ordering, fetch_order: atomic::Ordering, mut update: F
	) -> Result<Pointer<T, MUT>, Pointer<T, MUT>>
	where
		F: FnMut(Pointer<T, MUT>) -> Option<Pointer<T, MUT>>
	{
		match self.ptr.fetch_update(set_order, fetch_order, |ptr| {
			update(Pointer { ptr }).map(Pointer::ptr)
		}) {
			Ok(ptr) => Ok(Pointer { ptr }),
			Err(ptr) => Err(Pointer { ptr })
		}
	}
}
