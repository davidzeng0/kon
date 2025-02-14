use std::fmt::Write;

use super::*;

pub struct ContextError<C, E = crate::error::Error> {
	backtrace: Option<Backtrace>,
	context: C,
	error: E
}

struct Escaped<'a, 'b>(&'a mut Formatter<'b>);

impl Write for Escaped<'_, '_> {
	fn write_str(&mut self, str: &str) -> fmt::Result {
		write!(self.0, "{}", str.escape_debug())
	}
}

struct Quoted<C>(C);

impl<C: Display> Debug for Quoted<C> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		fmt.write_char('"')?;

		write!(Escaped(fmt), "{}", self.0)?;

		fmt.write_char('"')
	}
}

impl<C: Context> Debug for ContextError<C> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		fmt.debug_struct("Error")
			.field("context", &Quoted(&self.context))
			.field("source", &self.error)
			.finish()
	}
}

impl<C: Context> Display for ContextError<C> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		self.context.fmt(fmt)
	}
}

impl<C: Context> Error for ContextError<C> {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		Some(&self.error)
	}
}

impl<C: Context> ErrorImpl for ContextError<C> {
	fn kind(&self) -> ErrorKind {
		self.error.kind()
	}
}

impl<C: Context> ContextError<C> {
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

	/// # Safety
	/// See [`DynError::downcast_ptr`]
	unsafe fn downcast_ptr(this: MutNonNull<()>, type_id: TypeId) -> Option<MutNonNull<()>> {
		let this = this.cast::<Self>();

		/* Safety: valid ptr */
		let ptr = unsafe { ptr!(!null &mut this=>error) };

		/* Safety: valid ptr */
		unsafe { crate::error::Error::downcast_ptr(ptr, type_id) }
	}

	/// # Safety
	/// See [`DynError::downcast_owned`]
	unsafe fn downcast_owned(
		this: MutNonNull<()>, type_id: TypeId, out: MutPtr<MaybeUninit<()>>
	) -> bool {
		let this = this.cast::<DynError<ContextError<C, ManuallyDrop<crate::error::Error>>>>();

		/* Safety: valid ptr */
		let ptr = unsafe { ptr!(!null &mut this=>data.error) }.cast();

		/* Safety: valid ptr */
		if !unsafe { crate::error::Error::downcast_owned(ptr, type_id, out) } {
			return false;
		}

		/* Safety: drop the rest. guaranteed by caller. downcast_owned never panics */
		drop(unsafe { this.into_box() });

		true
	}

	/// # Safety
	/// See [`DynError::backtrace`]
	unsafe fn backtrace(this: MutNonNull<()>) -> Option<&'static Backtrace> {
		let this = this.cast::<Self>();

		/* Safety: valid ptr */
		let bt = unsafe { ptr!(this=>backtrace.as_ref()) };

		/* Safety: valid ptr */
		bt.or_else(|| unsafe { ptr!(this=>error.backtrace()) })
	}

	/// # Safety
	/// See [`DynError::drop`]
	unsafe fn drop(this: MutNonNull<()>) {
		let this = this.cast::<DynError<Self>>();

		/* Safety: guaranteed by caller */
		drop(unsafe { this.into_box() });
	}

	fn vtable() -> &'static ErrorVTable {
		&ErrorVTable {
			kind: Self::kind,
			meta: Self::meta,
			downcast_ptr: Self::downcast_ptr,
			downcast_owned: Self::downcast_owned,
			backtrace: Self::backtrace,
			drop: Self::drop
		}
	}

	pub fn new_dyn(context: C, error: crate::error::Error) -> MutNonNull<DynError<()>> {
		let backtrace = if error.backtrace().is_some() {
			None
		} else {
			capture_backtrace()
		};

		MutNonNull::from_box(Box::new(DynError {
			vtable: Self::vtable(),
			data: Self { backtrace, context, error }
		}))
		.cast()
	}
}
