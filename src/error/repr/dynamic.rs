#![allow(unreachable_pub, clippy::multiple_unsafe_ops_per_block)]

use super::*;

pub struct ErrorVTable {
	pub kind: unsafe fn(MutNonNull<()>) -> ErrorKind,
	pub meta: unsafe fn(MutNonNull<()>) -> MutNonNull<dyn ErrorImpl>,
	pub downcast_ptr: unsafe fn(MutNonNull<()>, TypeId) -> Option<MutNonNull<()>>,
	pub downcast_owned: unsafe fn(MutNonNull<()>, TypeId, MutPtr<MaybeUninit<()>>) -> bool,
	pub backtrace: unsafe fn(MutNonNull<()>) -> Option<&'static Backtrace>,
	pub drop: unsafe fn(MutNonNull<()>)
}

#[repr(C)]
pub struct DynError<T: ?Sized> {
	pub vtable: &'static ErrorVTable,
	pub data: T
}

impl DynError<()> {
	/// # Safety
	/// valid pointer
	unsafe fn into_parts(this: MutNonNull<Self>) -> (&'static ErrorVTable, MutNonNull<()>) {
		/* Safety: guaranteed by caller */
		unsafe { (ptr!(this=>vtable), ptr!(!null &mut this=>data)) }
	}

	/// # Safety
	/// valid pointer
	pub unsafe fn kind(this: MutNonNull<Self>) -> ErrorKind {
		/* Safety: guaranteed by caller */
		let (vtable, data) = unsafe { Self::into_parts(this) };

		/* Safety: valid ptr */
		unsafe { (vtable.kind)(data) }
	}

	/// # Safety
	/// valid pointer
	pub unsafe fn meta(this: MutNonNull<Self>) -> MutNonNull<dyn ErrorImpl> {
		/* Safety: guaranteed by caller */
		let (vtable, data) = unsafe { Self::into_parts(this) };

		/* Safety: valid ptr */
		unsafe { (vtable.meta)(data) }
	}

	/// # Safety
	/// valid pointer
	/// `type_id` must be valid
	pub unsafe fn downcast_ptr_type_id(
		this: MutNonNull<Self>, type_id: TypeId
	) -> Option<MutNonNull<()>> {
		/* Safety: guaranteed by caller */
		let (vtable, data) = unsafe { Self::into_parts(this) };

		/* Safety: valid ptr */
		let ptr = unsafe { (vtable.downcast_ptr)(data, type_id) }?;

		Some(ptr)
	}

	/// # Safety
	/// valid pointer
	pub unsafe fn downcast_ptr<T>(this: MutNonNull<Self>) -> Option<MutNonNull<T>>
	where
		T: ErrorBounds
	{
		/* Safety: guaranteed by caller */
		unsafe { Self::downcast_ptr_type_id(this, TypeId::of::<T>()) }.map(MutNonNull::cast)
	}

	/// # Safety
	/// valid pointers
	/// `type_id` must be valid
	/// ownership of the error is considered moved if and only if this function
	/// returns true
	pub unsafe fn downcast_owned_type_id(
		this: MutNonNull<Self>, type_id: TypeId, out: MutPtr<MaybeUninit<()>>
	) -> bool {
		/* Safety: guaranteed by caller */
		let (vtable, _) = unsafe { Self::into_parts(this) };

		/* Safety: valid ptr */
		unsafe { (vtable.downcast_owned)(this.cast(), type_id, out) }
	}

	/// # Safety
	/// valid pointer
	///
	/// See [`Self::downcast_owned_type_id`]
	pub unsafe fn downcast_owned<T>(this: MutNonNull<Self>) -> Option<T>
	where
		T: ErrorBounds
	{
		/* Safety: guaranteed by caller */
		let (vtable, _) = unsafe { Self::into_parts(this) };

		let mut result = MaybeUninit::uninit();

		/* Safety: valid ptr */
		let downcasted = unsafe {
			(vtable.downcast_owned)(this.cast(), TypeId::of::<T>(), ptr!(&mut result).cast())
		};

		/* Safety: downcast successful */
		downcasted.then(|| unsafe { result.assume_init() })
	}

	/// # Safety
	/// valid pointer
	pub unsafe fn backtrace<'a>(this: MutNonNull<Self>) -> Option<&'a Backtrace> {
		/* Safety: guaranteed by caller */
		let (vtable, data) = unsafe { Self::into_parts(this) };

		/* Safety: valid ptr */
		unsafe { (vtable.backtrace)(data) }
	}

	/// # Safety
	/// passes ownership of the pointer
	pub unsafe fn drop(this: MutNonNull<Self>) {
		/* Safety: guaranteed by caller */
		let (vtable, _) = unsafe { Self::into_parts(this) };

		/* Safety: valid ptr */
		unsafe { (vtable.drop)(this.cast()) }
	}
}

pub fn capture_backtrace() -> Option<Backtrace> {
	let mut backtrace = None;

	if backtrace.insert(Backtrace::capture()).status() != BacktraceStatus::Captured {
		backtrace = None;
	}

	backtrace
}

impl crate::error::Error {
	/// # Safety
	/// See [`DynError::downcast_ptr`]
	pub(super) unsafe fn downcast_ptr(
		this: MutNonNull<Self>, type_id: TypeId
	) -> Option<MutNonNull<()>> {
		/* Safety: we have atleast read access */
		let ErrorData::Custom(CustomRef(ptr, _)) = (unsafe { ptr!(this=>0.data()) }) else {
			return None;
		};

		/* Safety: valid ptr */
		unsafe { DynError::downcast_ptr_type_id(ptr, type_id) }
	}

	/// # Safety
	/// See [`DynError::downcast_owned`]
	pub(super) unsafe fn downcast_owned(
		this: MutNonNull<Self>, type_id: TypeId, out: MutPtr<MaybeUninit<()>>
	) -> bool {
		/* Safety: we have atleast read access */
		let ErrorData::Custom(CustomRef(ptr, _)) = (unsafe { ptr!(this=>0.data()) }) else {
			return false;
		};

		/* Safety: valid ptr */
		unsafe { DynError::downcast_owned_type_id(ptr, type_id, out) }
	}
}
