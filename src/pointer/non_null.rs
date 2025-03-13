use super::*;

#[repr(transparent)]
pub struct NonNullPtr<T: ?Sized, const MUT: bool> {
	ptr: pointer::NonNull<T>
}

pub type NonNull<T> = NonNullPtr<T, false>;
pub type MutNonNull<T> = NonNullPtr<T, true>;

impl<T: ?Sized, const MUT: bool> NonNullPtr<T, MUT> {
	/// # Safety
	/// `ptr` must not be null
	#[must_use]
	pub const unsafe fn new_unchecked(ptr: Pointer<T, MUT>) -> Self {
		/* Safety: guaranteed by caller */
		let ptr = unsafe { pointer::NonNull::new_unchecked(ptr.ptr()) };

		Self { ptr }
	}

	#[must_use]
	pub fn new(ptr: Pointer<T, MUT>) -> Option<Self> {
		pointer::NonNull::new(ptr.ptr()).map(|ptr| Self { ptr })
	}

	#[must_use]
	pub const fn as_pointer(self) -> Pointer<T, MUT> {
		Pointer { ptr: self.ptr.as_ptr() }
	}

	#[must_use]
	pub const fn as_ptr(self) -> *const T {
		self.ptr.as_ptr()
	}

	#[must_use]
	pub const fn cast<T2>(self) -> NonNullPtr<T2, MUT> {
		NonNullPtr { ptr: self.ptr.cast() }
	}

	#[must_use]
	pub fn addr(self) -> usize {
		self.as_pointer().addr()
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	#[must_use]
	#[allow(clippy::missing_const_for_fn)]
	pub unsafe fn as_ref<'a>(self) -> &'a T {
		/* Safety: guaranteed by caller */
		unsafe { self.as_pointer().as_ref() }
	}
}

impl<T, const MUT: bool> NonNullPtr<T, MUT> {
	wrapper_functions! {
		inner = self.as_pointer();

		pub fn align_offset(self, align: usize) -> usize;
		pub unsafe fn read(self) -> T;
	}

	#[must_use]
	pub const fn dangling() -> Self {
		Self { ptr: pointer::NonNull::dangling() }
	}

	/// # Safety
	///  See the pointer `add` function
	#[must_use]
	pub const unsafe fn add(mut self, count: usize) -> Self {
		/* Safety: guaranteed by caller */
		self.ptr = unsafe { self.ptr.add(count) };
		self
	}

	/// # Safety
	///
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
		#[allow(clippy::multiple_unsafe_ops_per_block)]
		unsafe {
			Self::new_unchecked(offset.offset(self.as_pointer()))
		}
	}

	/// # Safety
	/// `value` must be non zero
	#[must_use]
	pub const unsafe fn from_addr_unchecked(value: usize) -> Self {
		/* Safety: guaranteed by caller */
		unsafe { Self::new_unchecked(Pointer::from_addr(value)) }
	}

	#[must_use]
	pub const fn from_addr(value: NonZeroUsize) -> Self {
		/* Safety: value is non zero */
		unsafe { Self::from_addr_unchecked(value.get()) }
	}
}

impl<T: ?Sized> NonNull<T> {
	#[must_use]
	pub const fn cast_mut(self) -> MutNonNull<T> {
		MutNonNull { ptr: self.ptr }
	}
}

impl<T: ?Sized> MutNonNull<T> {
	wrapper_functions! {
		inner = self.as_pointer();

		pub unsafe fn drop_in_place(self);
	}

	#[must_use]
	pub const fn cast_const(self) -> NonNull<T> {
		NonNull { ptr: self.ptr }
	}

	#[must_use]
	pub const fn as_mut_ptr(self) -> *mut T {
		self.ptr.as_ptr()
	}

	/// # Safety
	/// Caller must enforce aliasing rules. See std::ptr::as_ref
	#[must_use]
	pub unsafe fn as_mut<'a>(self) -> &'a mut T {
		/* Safety: guaranteed by caller */
		unsafe { self.as_pointer().as_mut() }
	}

	#[must_use]
	pub fn from_box(ptr: Box<T>) -> Self {
		/* Safety: Box::into_raw is always non null */
		unsafe { Self::new_unchecked(Box::into_raw(ptr).into()) }
	}

	/// # Safety
	/// See [`Box::from_raw`]
	#[must_use]
	pub unsafe fn into_box(self) -> Box<T> {
		/* Safety: Box::into_raw is always non null */
		unsafe { Box::from_raw(self.as_mut_ptr()) }
	}
}

impl<T> MutNonNull<T> {
	wrapper_functions! {
		inner = self.as_pointer();

		pub unsafe fn write_bytes(self, val: u8, count: usize);
		pub unsafe fn write(self, value: T);
	}
}

impl<T, const MUT: bool> NonNullPtr<[T], MUT> {
	wrapper_functions! {
		inner = self.as_pointer();

		#[must_use]
		pub fn len(self) -> usize;

		#[must_use]
		pub fn is_empty(self) -> bool;

		#[must_use]
		pub unsafe fn into_iter(self) -> PointerIterator<T, MUT>;
	}

	#[must_use]
	pub fn slice_from_raw_parts(data: NonNullPtr<T, MUT>, len: usize) -> Self {
		Self {
			ptr: pointer::NonNull::slice_from_raw_parts(data.ptr, len)
		}
	}
}

impl<T: ?Sized, const MUT: bool> Clone for NonNullPtr<T, MUT> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized, const MUT: bool> Copy for NonNullPtr<T, MUT> {}

impl<T: ?Sized> From<&T> for NonNull<T> {
	fn from(ptr: &T) -> Self {
		#[allow(trivial_casts)]
		Self { ptr: ptr.into() }
	}
}

impl<T: ?Sized> From<&mut T> for MutNonNull<T> {
	fn from(ptr: &mut T) -> Self {
		Self { ptr: ptr.into() }
	}
}

impl<T: ?Sized, const MUT: bool> PartialEq for NonNullPtr<T, MUT> {
	fn eq(&self, other: &Self) -> bool {
		self.as_pointer().eq(&other.as_pointer())
	}
}

impl<T: ?Sized, const MUT: bool> Eq for NonNullPtr<T, MUT> {}

impl<T: ?Sized, const MUT: bool> PartialOrd for NonNullPtr<T, MUT> {
	fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl<T: ?Sized, const MUT: bool> Ord for NonNullPtr<T, MUT> {
	#[allow(ambiguous_wide_pointer_comparisons)]
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		self.ptr.cmp(&other.ptr)
	}
}

impl<T: Sized, const MUT: bool> Default for NonNullPtr<T, MUT> {
	fn default() -> Self {
		Self::dangling()
	}
}

impl<T: ?Sized, const MUT: bool> Debug for NonNullPtr<T, MUT> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		Debug::fmt(&self.ptr, fmt)
	}
}

impl<T: ?Sized, const MUT: bool> fmt::Pointer for NonNullPtr<T, MUT> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> Result {
		fmt::Pointer::fmt(&self.ptr, fmt)
	}
}
