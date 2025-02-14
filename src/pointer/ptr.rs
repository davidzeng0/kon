use super::*;

#[repr(transparent)]
pub struct Pointer<T: ?Sized, const MUT: bool> {
	pub(crate) ptr: *const T
}

pub type Ptr<T> = Pointer<T, false>;
pub type MutPtr<T> = Pointer<T, true>;

impl<T: ?Sized, const MUT: bool> Pointer<T, MUT> {
	wrapper_functions! {
		inner = self.ptr;

		pub fn is_null(self) -> bool;
	}

	pub(crate) const fn ptr(self) -> *mut T {
		self.ptr.cast_mut()
	}

	#[must_use]
	pub const fn as_ptr(self) -> *const T {
		self.ptr
	}

	#[must_use]
	pub const fn cast<T2>(self) -> Pointer<T2, MUT> {
		Pointer { ptr: self.ptr.cast() }
	}

	#[must_use]
	pub fn addr(self) -> usize {
		self.as_ptr().cast::<()>() as usize
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	#[must_use]
	#[allow(clippy::missing_const_for_fn)]
	pub unsafe fn as_ref<'a>(self) -> &'a T {
		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(!self.is_null()) };

		/* Safety: guaranteed by caller */
		unsafe { &*self.ptr }
	}
}

impl<T, const MUT: bool> Pointer<T, MUT> {
	wrapper_functions! {
		inner = self.ptr;

		pub fn align_offset(self, align: usize) -> usize;
		pub unsafe fn read(self) -> T;
	}

	/// # Safety
	/// See the pointer `add` function
	#[must_use]
	pub const unsafe fn add(mut self, count: usize) -> Self {
		/* Safety: guaranteed by caller */
		self.ptr = unsafe { self.ptr.add(count) };
		self
	}

	/// # Safety
	/// See the pointer `sub` function
	#[must_use]
	pub const unsafe fn sub(mut self, count: usize) -> Self {
		/* Safety: guaranteed by caller */
		self.ptr = unsafe { self.ptr.sub(count) };
		self
	}

	/// # Safety
	/// See the pointer `offset` function
	#[must_use]
	#[allow(clippy::impl_trait_in_params)]
	pub unsafe fn offset(self, offset: impl internal::PointerOffset) -> Self {
		/* Safety: guaranteed by caller */
		unsafe { offset.offset(self) }
	}

	#[must_use]
	pub const fn from_addr(value: usize) -> Self {
		Self { ptr: value as *mut _ }
	}

	#[must_use]
	pub const fn null() -> Self {
		Self { ptr: null_mut() }
	}

	/// # Safety
	/// `self` must not be null
	#[must_use]
	pub const unsafe fn cast_nonnull(self) -> NonNullPtr<T, MUT> {
		/* Safety: guaranteed by caller */
		unsafe { NonNullPtr::new_unchecked(self) }
	}
}

impl<T: ?Sized> Ptr<T> {
	#[must_use]
	pub const fn cast_mut(self) -> MutPtr<T> {
		MutPtr { ptr: self.ptr }
	}
}

impl<T: ?Sized> MutPtr<T> {
	wrapper_functions! {
		inner = self.ptr();

		pub unsafe fn drop_in_place(self);
	}

	#[must_use]
	pub const fn cast_const(self) -> Ptr<T> {
		Ptr { ptr: self.ptr }
	}

	#[must_use]
	pub const fn as_mut_ptr(self) -> *mut T {
		self.ptr()
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	#[must_use]
	pub unsafe fn as_mut<'a>(self) -> &'a mut T {
		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(!self.is_null()) };

		/* Safety: guaranteed by caller */
		unsafe { &mut *self.ptr() }
	}
}

impl<T> MutPtr<T> {
	wrapper_functions! {
		inner = self.ptr();

		pub unsafe fn write_bytes(self, val: u8, count: usize);
		pub unsafe fn write(self, value: T);
	}

	/// # Safety
	/// See [`std::ptr::copy`]
	pub unsafe fn copy_from(self, src: Ptr<T>, count: usize) {
		/* Safety: guaranteed by caller */
		unsafe { self.ptr().copy_from(src.ptr, count) }
	}

	/// # Safety
	/// See [`std::ptr::copy`]
	pub unsafe fn copy_from_nonoverlapping(self, src: Ptr<T>, count: usize) {
		/* Safety: guaranteed by caller */
		unsafe { self.ptr().copy_from_nonoverlapping(src.ptr, count) }
	}
}

pub struct PointerIterator<T, const MUT: bool> {
	start: Pointer<T, MUT>,
	end: Pointer<T, MUT>
}

impl<T, const MUT: bool> Iterator for PointerIterator<T, MUT> {
	type Item = Pointer<T, MUT>;

	fn next(&mut self) -> Option<Self::Item> {
		let cur = self.start;

		if cur < self.end {
			/* Safety: start < end */
			unsafe { self.start = self.start.add(1) };

			Some(cur)
		} else {
			None
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		#[allow(clippy::cast_sign_loss)]
		/* Safety: start < end */
		let len = unsafe { self.end.as_ptr().offset_from(self.start.as_ptr()) } as usize;

		(len, Some(len))
	}
}

impl<T, const MUT: bool> DoubleEndedIterator for PointerIterator<T, MUT> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.end > self.start {
			/* Safety: start < end */
			unsafe { self.end = self.end.sub(1) };

			Some(self.end)
		} else {
			None
		}
	}
}

impl<T, const MUT: bool> ExactSizeIterator for PointerIterator<T, MUT> {}

impl<T, const MUT: bool> Pointer<[T], MUT> {
	wrapper_functions! {
		inner = self.ptr;

		#[must_use]
		pub fn len(self) -> usize;

		#[must_use]
		pub fn is_empty(self) -> bool;
	}

	#[must_use]
	pub fn slice_from_raw_parts(data: Pointer<T, MUT>, len: usize) -> Self {
		Self { ptr: slice_from_raw_parts_mut(data.ptr(), len) }
	}

	/// # Safety
	/// The slice must be one contiguous memory segment
	#[must_use]
	pub unsafe fn into_iter(self) -> PointerIterator<T, MUT> {
		let start = self.cast::<T>();

		/* Safety: guaranteed by caller */
		let end = unsafe { start.add(self.len()) };

		PointerIterator { start, end }
	}
}

impl<T> MutPtr<[T]> {
	/// # Safety
	/// See [`std::ptr::copy`]
	pub unsafe fn copy_from(self, src: Ptr<T>, count: usize) {
		/* Safety: guaranteed by caller */
		unsafe { self.cast::<T>().copy_from(src, count) }
	}

	/// # Safety
	/// See [`std::ptr::copy`]
	pub unsafe fn copy_from_nonoverlapping(self, src: Ptr<T>, count: usize) {
		/* Safety: guaranteed by caller */
		unsafe { self.cast::<T>().copy_from_nonoverlapping(src, count) }
	}
}

/* derive Clone requires that T: Clone */
impl<T: ?Sized, const MUT: bool> Clone for Pointer<T, MUT> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized, const MUT: bool> Copy for Pointer<T, MUT> {}

impl<T: ?Sized, const MUT: bool> From<NonNullPtr<T, MUT>> for Pointer<T, MUT> {
	fn from(value: NonNullPtr<T, MUT>) -> Self {
		value.as_pointer()
	}
}

impl<T: ?Sized> From<*const T> for Ptr<T> {
	fn from(ptr: *const T) -> Self {
		Self { ptr: ptr.cast_mut() }
	}
}

impl<T: ?Sized> From<&T> for Ptr<T> {
	fn from(ptr: &T) -> Self {
		#[allow(trivial_casts)]
		Self { ptr: (ptr as *const T).cast_mut() }
	}
}

impl<T: ?Sized> From<*mut T> for MutPtr<T> {
	fn from(ptr: *mut T) -> Self {
		Self { ptr }
	}
}

impl<T: ?Sized> From<&mut T> for MutPtr<T> {
	fn from(ptr: &mut T) -> Self {
		Self { ptr }
	}
}

impl<T: ?Sized, const MUT: bool> PartialEq for Pointer<T, MUT> {
	fn eq(&self, other: &Self) -> bool {
		pointer::eq(self.ptr, other.ptr)
	}
}

impl<T: ?Sized, const MUT: bool> Eq for Pointer<T, MUT> {}

impl<T: ?Sized, const MUT: bool> PartialOrd for Pointer<T, MUT> {
	fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl<T: ?Sized, const MUT: bool> Ord for Pointer<T, MUT> {
	#[allow(ambiguous_wide_pointer_comparisons)]
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		self.ptr.cmp(&other.ptr)
	}
}

impl<T: Sized, const MUT: bool> Default for Pointer<T, MUT> {
	fn default() -> Self {
		Self::null()
	}
}

impl<T: ?Sized, const MUT: bool> Debug for Pointer<T, MUT> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		Debug::fmt(&self.ptr, fmt)
	}
}

impl<T: ?Sized, const MUT: bool> fmt::Pointer for Pointer<T, MUT> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		fmt::Pointer::fmt(&self.ptr, fmt)
	}
}

#[macro_export]
macro_rules! ptr {
	(*$ptr:expr) => {
		*$crate::pointer::internal::AsPointer::as_pointer(&$ptr)
	};

	(&$value:expr) => {
		$crate::pointer::Pointer::from(::std::ptr::addr_of!($value))
	};

	(&mut $value:expr) => {
		$crate::pointer::Pointer::from(::std::ptr::addr_of_mut!($value))
	};

	(!null &$value:expr) => {
		({
			const fn as_non_null<T>(ptr: $crate::pointer::Ptr<T>) -> $crate::pointer::NonNull<T> {
				/* Safety: reference of a value is always non null */
				unsafe { ptr.cast_nonnull() }
			}

			as_non_null::<_>
		})($crate::pointer::ptr!(&$value))
	};

	(!null &mut $value:expr) => {
		({
			const fn as_non_null<T>(ptr: $crate::pointer::MutPtr<T>) -> $crate::pointer::MutNonNull<T> {
				/* Safety: reference of a value is always non null */
				unsafe { ptr.cast_nonnull() }
			}

			as_non_null::<_>
		})($crate::pointer::ptr!(&mut $value))
	};

	(&$ptr:expr => $($expr:tt)*) => {
		$crate::pointer::ptr!(
			&$crate::pointer::ptr!($ptr => $($expr)*)
		)
	};

	(&mut $ptr:expr => $($expr:tt)*) => {
		$crate::pointer::ptr!(
			&mut $crate::pointer::ptr!($ptr => $($expr)*)
		)
	};

	(!null &$ptr:expr => $($expr:tt)*) => {
		$crate::pointer::ptr!(
			!null &$crate::pointer::ptr!($ptr => $($expr)*)
		)
	};

	(!null &mut $ptr:expr => $($expr:tt)*) => {
		$crate::pointer::ptr!(
			!null &mut $crate::pointer::ptr!($ptr => $($expr)*)
		)
	};

	($ptr:expr => [$index:expr] $($expr:tt)*) => {
		$crate::pointer::ptr!(*
			$crate::pointer::internal::PointerIndex::index($ptr, $index)
		) $($expr)*
	};

	($ptr:expr => $($expr:tt)*) => {
		$crate::pointer::ptr!(*$ptr).$($expr)*
	};

	($ref:expr) => {
		$crate::pointer::Pointer::from($ref)
	};

	(!null $ref:expr) => {
		$crate::pointer::NonNullPtr::from($ref)
	};
}

pub use ptr;
