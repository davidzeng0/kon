use super::*;

pub struct CustomError<E, K> {
	backtrace: Option<Backtrace>,
	error: E,
	kind: K
}

impl<E: Display, K> Display for CustomError<E, K> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Display::fmt(&self.error, fmt)
	}
}

impl<E: Debug, K> Debug for CustomError<E, K> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Debug::fmt(&self.error, fmt)
	}
}

impl<E: Error, K> Error for CustomError<E, K> {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.error.source()
	}
}

impl<E: ErrorImpl, K: CompactErrorKind> ErrorImpl for CustomError<E, K> {
	fn kind(&self) -> ErrorKind {
		self.kind.kind(&self.error)
	}
}

impl<E, K> CustomError<E, K> {
	/// # Safety
	/// See [`DynError::downcast_ptr`]
	unsafe fn downcast_ptr<I>(this: MutNonNull<()>, type_id: TypeId) -> Option<MutNonNull<()>>
	where
		I: ErrorBounds
	{
		let this = this.cast::<Self>();

		if type_id != TypeId::of::<I>() {
			return None;
		}

		/* Safety: valid ptr */
		let ptr = unsafe { ptr!(!null &mut this=>error) };

		Some(ptr.cast())
	}

	/// # Safety
	/// See [`DynError::downcast_owned`]
	unsafe fn downcast_owned<I>(
		this: MutNonNull<()>, type_id: TypeId, out: MutPtr<MaybeUninit<()>>
	) -> bool
	where
		I: ErrorBounds
	{
		let this = this.cast::<DynError<CustomError<ManuallyDrop<I>, K>>>();
		let out = out.cast::<MaybeUninit<I>>();

		if type_id != TypeId::of::<I>() {
			return false;
		}

		/* Safety: get the inner object */
		let error = unsafe { ManuallyDrop::take(&mut ptr!(this=>data.error)) };

		/* Safety: move the error object */
		unsafe { ptr!(out=>write(error)) };

		/* Safety: drop the rest. guaranteed by caller */
		drop(unsafe { this.into_box() });

		true
	}

	/// # Safety
	/// See [`DynError::backtrace`]
	unsafe fn backtrace(this: MutNonNull<()>) -> Option<&'static Backtrace> {
		let this = this.cast::<Self>();

		/* Safety: valid ptr */
		unsafe { ptr!(this=>backtrace.as_ref()) }
	}

	/// # Safety
	/// See [`DynError::drop`]
	unsafe fn drop(this: MutNonNull<()>) {
		let this = this.cast::<DynError<Self>>();

		/* Safety: guaranteed by caller */
		drop(unsafe { this.into_box() });
	}

	fn into_dyn(self, vtable: &'static ErrorVTable) -> MutNonNull<DynError<()>> {
		MutNonNull::from_box(Box::new(DynError { vtable, data: self })).cast()
	}
}

macro_rules! wrapper_type {
	($name:ident $($bounds:tt)*) => {
		#[repr(transparent)]
		struct $name<E>(pub E);

		impl<E: Display> Display for $name<E> {
			fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
				Display::fmt(&self.0, fmt)
			}
		}

		impl<E: Debug> Debug for $name<E> {
			fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
				Debug::fmt(&self.0, fmt)
			}
		}

		impl<E: ErrorBounds $($bounds)*> ErrorImpl for $name<E> {
			fn kind(&self) -> ErrorKind {
				ErrorKind::Other
			}
		}
	}
}

wrapper_type!(Basic);

impl<E: ErrorBounds> Error for Basic<E> {}

impl<E: ErrorBounds, K: CompactErrorKind> CustomError<E, K> {
	pub fn new_basic(error: E, kind: K) -> MutNonNull<DynError<()>> {
		CustomError::new(Basic(error), kind).into_dyn(&ErrorVTable {
			kind: |_| ErrorKind::Other,
			meta: CustomError::<Basic<E>, K>::meta,
			downcast_ptr: Self::downcast_ptr::<E>,
			downcast_owned: Self::downcast_owned::<E>,
			backtrace: Self::backtrace,
			drop: Self::drop
		})
	}
}

wrapper_type!(Std + Error);

impl<E: ErrorBounds + Error> Error for Std<E> {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.0.source()
	}
}

impl<E: ErrorBounds + Error, K: CompactErrorKind> CustomError<E, K> {
	pub fn new_std(error: E, kind: K) -> MutNonNull<DynError<()>> {
		CustomError::new(Std(error), kind).into_dyn(&ErrorVTable {
			kind: |_| ErrorKind::Other,
			meta: CustomError::<Std<E>, K>::meta,
			downcast_ptr: Self::downcast_ptr::<E>,
			downcast_owned: Self::downcast_owned::<E>,
			backtrace: Self::backtrace,
			drop: Self::drop
		})
	}
}

#[repr(transparent)]
struct Boxed(pub BoxedError);

impl Display for Boxed {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Display::fmt(&self.0, fmt)
	}
}

impl Debug for Boxed {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Debug::fmt(&self.0, fmt)
	}
}

impl Error for Boxed {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.0.source()
	}
}

impl ErrorImpl for Boxed {
	fn kind(&self) -> ErrorKind {
		ErrorKind::Other
	}
}

impl<K: CompactErrorKind> CustomError<BoxedError, K> {
	pub fn new_boxed(error: BoxedError, kind: K) -> MutNonNull<DynError<()>> {
		CustomError::new(Boxed(error), kind).into_dyn(&ErrorVTable {
			kind: |_| ErrorKind::Other,
			meta: CustomError::<Boxed, K>::meta,
			downcast_ptr: Self::downcast_ptr::<BoxedError>,
			downcast_owned: Self::downcast_owned::<BoxedError>,
			backtrace: Self::backtrace,
			drop: Self::drop
		})
	}
}

impl<E: ErrorImpl, K: CompactErrorKind> CustomError<E, K> {
	/// # Safety
	/// See [`DynError::kind`]
	unsafe fn kind(this: MutNonNull<()>) -> ErrorKind {
		let this = this.cast::<Self>();

		/* Safety: valid ptr */
		unsafe { ptr!(this=>error.kind()) }
	}

	/// # Safety
	/// See [`DynError::meta`]
	unsafe fn meta(this: MutNonNull<()>) -> MutNonNull<dyn ErrorImpl> {
		let this = this.cast::<Self>().as_mut_ptr() as *mut dyn ErrorImpl;

		/* Safety: `this` is non-null */
		unsafe { MutNonNull::new_unchecked(this.into()) }
	}

	fn new(error: E, kind: K) -> Self {
		Self { backtrace: capture_backtrace(), error, kind }
	}

	pub fn new_error_impl(error: E, kind: K) -> MutNonNull<DynError<()>> {
		Self::new(error, kind).into_dyn(&ErrorVTable {
			kind: Self::kind,
			meta: Self::meta,
			downcast_ptr: Self::downcast_ptr::<E>,
			downcast_owned: Self::downcast_owned::<E>,
			backtrace: Self::backtrace,
			drop: Self::drop
		})
	}
}
