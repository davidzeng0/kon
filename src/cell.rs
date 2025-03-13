use std::cell;
use std::ops::{Deref, DerefMut};

use crate::macros::wrapper_functions;
use crate::pointer::*;

#[derive(Clone)]
pub struct Cell<T: Copy>(cell::Cell<T>);

impl<T: Copy> Cell<T> {
	wrapper_functions! {
		inner = self.0;

		pub fn into_inner(self) -> T;
	}

	#[must_use]
	pub const fn new(value: T) -> Self {
		Self(cell::Cell::new(value))
	}

	pub fn update<F>(&self, update: F) -> T
	where
		F: FnOnce(T) -> T
	{
		let value = update(self.get());

		self.set(value);

		value
	}
}

impl<T: Copy> Deref for Cell<T> {
	type Target = cell::Cell<T>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Copy> DerefMut for Cell<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<T: Copy + PartialEq> PartialEq<T> for Cell<T> {
	fn eq(&self, other: &T) -> bool {
		self.get().eq(other)
	}
}

impl<T: Copy + PartialOrd> PartialOrd<T> for Cell<T> {
	fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
		self.get().partial_cmp(other)
	}
}

#[repr(transparent)]
pub struct UnsafeCell<T: ?Sized> {
	data: cell::UnsafeCell<T>
}

impl<T: ?Sized> UnsafeCell<T> {
	pub const fn new(data: T) -> Self
	where
		T: Sized
	{
		Self { data: cell::UnsafeCell::new(data) }
	}

	pub fn get(&self) -> MutPtr<T> {
		self.data.get().into()
	}

	pub fn get_mut(&mut self) -> &mut T {
		self.data.get_mut()
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	pub unsafe fn as_ref<'a>(&self) -> &'a T {
		/* Safety: guaranteed by caller */
		unsafe { self.get().as_ref() }
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	pub unsafe fn as_mut<'a>(&self) -> &'a mut T {
		/* Safety: guaranteed by caller */
		unsafe { self.get().as_mut() }
	}

	pub fn into_inner(self) -> T
	where
		T: Sized
	{
		self.data.into_inner()
	}
}

impl<T: Pin + ?Sized> Pin for UnsafeCell<T> {
	unsafe fn pin(&mut self) {
		/* Safety: we are being pinned */
		unsafe { self.get_mut().pin() };
	}
}
