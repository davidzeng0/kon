use super::*;
#[cfg(feature = "cell")]
use crate::cell::UnsafeCell;

sealed_trait!();

pub trait AsPointer: Sealed {
	type Target;

	fn as_pointer(&self) -> Self::Target;
}

impl<T: ?Sized, const MUT: bool> Sealed for Pointer<T, MUT> {}

impl<T: ?Sized> AsPointer for Ptr<T> {
	type Target = *const T;

	fn as_pointer(&self) -> *const T {
		self.ptr
	}
}

impl<T: ?Sized> AsPointer for MutPtr<T> {
	type Target = *mut T;

	fn as_pointer(&self) -> *mut T {
		self.ptr()
	}
}

impl<T: ?Sized, const MUT: bool> Sealed for NonNullPtr<T, MUT> {}

impl<T: ?Sized> AsPointer for NonNull<T> {
	type Target = *const T;

	fn as_pointer(&self) -> *const T {
		self.as_ptr()
	}
}

impl<T: ?Sized> AsPointer for MutNonNull<T> {
	type Target = *mut T;

	fn as_pointer(&self) -> *mut T {
		self.as_mut_ptr()
	}
}

#[cfg(feature = "cell")]
impl<T: ?Sized> Sealed for UnsafeCell<T> {}

#[cfg(feature = "cell")]
impl<T: ?Sized> AsPointer for UnsafeCell<T> {
	type Target = *mut T;

	fn as_pointer(&self) -> *mut T {
		self.get().as_pointer()
	}
}

pub trait PointerOffset {
	/// # Safety
	/// See the pointer `offset` function
	unsafe fn offset<T, const MUT: bool>(self, pointer: Pointer<T, MUT>) -> Pointer<T, MUT>;
}

impl PointerOffset for usize {
	unsafe fn offset<T, const MUT: bool>(self, mut pointer: Pointer<T, MUT>) -> Pointer<T, MUT> {
		/* Safety: guaranteed by caller */
		pointer.ptr = unsafe { pointer.ptr.add(self) };
		pointer
	}
}

impl PointerOffset for isize {
	unsafe fn offset<T, const MUT: bool>(self, mut pointer: Pointer<T, MUT>) -> Pointer<T, MUT> {
		/* Safety: guaranteed by caller */
		pointer.ptr = unsafe { pointer.ptr.offset(self) };
		pointer
	}
}

pub trait PointerIndex<const MUT: bool, Idx> {
	type Output: ?Sized;

	/// # Safety
	/// See the pointer `offset` function
	unsafe fn index(self, index: Idx) -> Pointer<Self::Output, MUT>;
}

impl<const MUT: bool, T> PointerIndex<MUT, usize> for Pointer<[T], MUT> {
	type Output = T;

	unsafe fn index(self, index: usize) -> Pointer<T, MUT> {
		/* Safety: guaranteed by caller */
		unsafe { self.cast().add(index) }
	}
}
