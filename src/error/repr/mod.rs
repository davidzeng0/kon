#![allow(unreachable_pub, clippy::multiple_unsafe_ops_per_block)]

use std::any::TypeId;
use std::backtrace::{Backtrace, BacktraceStatus};
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::marker::PhantomData;
use std::mem::{forget, transmute, ManuallyDrop, MaybeUninit};

use crate::macros::const_assert;
use crate::pointer::*;

mod context;
mod custom;
mod dynamic;

use self::context::*;
use self::custom::*;
use self::dynamic::*;
use super::internal::*;
use super::private::*;
use super::{BoxedError, Context, ErrorKind, OsError};

pub trait CompactErrorKind: Send + Sync + 'static {
	fn kind<E>(&self, error: &E) -> ErrorKind
	where
		E: ErrorImpl;
}

impl CompactErrorKind for () {
	fn kind<E>(&self, error: &E) -> ErrorKind
	where
		E: ErrorImpl
	{
		error.kind()
	}
}

impl CompactErrorKind for ErrorKind {
	fn kind<E>(&self, _: &E) -> ErrorKind
	where
		E: ErrorImpl
	{
		*self
	}
}

pub struct CustomRef<'a, const MUT: bool = false>(MutNonNull<DynError<()>>, PhantomData<&'a ()>);

impl<'a, const MUT: bool> CustomRef<'a, MUT> {
	/// # Safety
	/// valid ptr
	const unsafe fn from(ptr: MutNonNull<DynError<()>>) -> Self {
		Self(ptr, PhantomData)
	}

	pub fn kind(&self) -> ErrorKind {
		/* Safety: guaranteed by constructor */
		unsafe { DynError::kind(self.0) }
	}

	pub fn meta(&self) -> &'a dyn ErrorImpl {
		/* Safety: guaranteed by constructor */
		unsafe { DynError::meta(self.0).as_ref() }
	}

	pub fn downcast_ref<T>(&self) -> Option<&'a T>
	where
		T: ErrorBounds
	{
		/* Safety: guaranteed by constructor */
		unsafe { DynError::downcast_ptr(self.0).map(|ptr| ptr.as_ref()) }
	}

	pub fn backtrace(&self) -> Option<&'a Backtrace> {
		/* Safety: guaranteed by constructor */
		unsafe { DynError::backtrace(self.0) }
	}

	pub fn source(&self) -> Option<&'a (dyn Error + 'static)> {
		self.meta().source()
	}
}

impl<'a> CustomRef<'a, true> {
	pub fn downcast_mut<T>(&mut self) -> Option<&'a mut T>
	where
		T: ErrorBounds
	{
		/* Safety: guaranteed by constructor */
		unsafe { DynError::downcast_ptr(self.0).map(|ptr| ptr.as_mut()) }
	}
}

struct Causes<'a>(&'a dyn Error);

impl Debug for Causes<'_> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		let mut source = Some(self.0);
		let mut count = 0usize;

		write!(fmt, "\n\nCaused by:")?;

		while let Some(error) = source {
			source = error.source();

			write!(fmt, "\n{: >4}: {}", count, error)?;

			#[allow(clippy::arithmetic_side_effects)]
			(count += 1);
		}

		Ok(())
	}
}

impl Display for Causes<'_> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		let mut source = Some(self.0);

		while let Some(error) = source {
			source = error.source();

			write!(fmt, ": {}", error)?;
		}

		Ok(())
	}
}

impl<const MUT: bool> Debug for CustomRef<'_, MUT> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		let meta = self.meta();
		let alternate = fmt.alternate();

		if !alternate {
			Display::fmt(meta, fmt)?;

			if let Some(source) = meta.source() {
				Debug::fmt(&Causes(source), fmt)?;
			}

			if let Some(bt) = self.backtrace() {
				write!(fmt, "\n\nBack trace:\n{}", bt)?;
			}

			Ok(())
		} else {
			let mut dbg = fmt.debug_struct("Custom");

			dbg.field("kind", &self.kind()).field("error", &meta);

			if let Some(bt) = self.backtrace() {
				dbg.field("backtrace", bt);
			}

			dbg.finish()
		}
	}
}

impl<const MUT: bool> Display for CustomRef<'_, MUT> {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		let meta = self.meta();

		Display::fmt(meta, fmt)?;

		if fmt.alternate() {
			if let Some(source) = meta.source() {
				Display::fmt(&Causes(source), fmt)?;
			}
		}

		Ok(())
	}
}

pub struct Custom(MutNonNull<DynError<()>>);

impl Custom {
	pub fn new_boxed<K>(error: BoxedError, kind: K) -> Self
	where
		K: CompactErrorKind
	{
		Self(CustomError::new_boxed(error, kind))
	}

	pub fn new_basic<E, K>(error: E, kind: K) -> Self
	where
		E: ErrorBounds,
		K: CompactErrorKind
	{
		Self(CustomError::new_basic(error, kind))
	}

	pub fn new_std<E, K>(error: E, kind: K) -> Self
	where
		E: ErrorBounds + Error,
		K: CompactErrorKind
	{
		Self(CustomError::new_std(error, kind))
	}

	pub fn new_error_impl<E, K>(error: E, kind: K) -> Self
	where
		E: ErrorImpl,
		K: CompactErrorKind
	{
		Self(CustomError::new_error_impl(error, kind))
	}

	pub fn new_context<C: Context>(context: C, error: super::Error) -> Self {
		Self(ContextError::new_dyn(context, error))
	}

	pub fn downcast<E>(self) -> Result<E, Self>
	where
		E: ErrorBounds
	{
		/* Safety: guaranteed by constructor */
		let result = unsafe { DynError::downcast_owned(self.0) };

		match result {
			Some(error) => {
				forget(self);

				Ok(error)
			}

			None => Err(self)
		}
	}
}

impl Drop for Custom {
	fn drop(&mut self) {
		/* Safety: we are being dropped */
		unsafe { DynError::drop(self.0) };
	}
}

const_assert!(align_of::<Custom>() >= 4);

#[derive(Clone, Copy)]
pub struct SimpleMessage {
	pub kind: ErrorKind,
	pub message: &'static str
}

impl Debug for SimpleMessage {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		fmt.debug_struct("Error")
			.field("kind", &self.kind)
			.field("message", &self.message)
			.finish()
	}
}

impl Display for SimpleMessage {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Display::fmt(self.message, fmt)
	}
}

const_assert!(align_of::<SimpleMessage>() >= 4);

pub enum ErrorData<C> {
	Os(OsError),
	Simple(ErrorKind),
	SimpleMessage(&'static SimpleMessage),
	Custom(C)
}

#[repr(usize)]
enum Tag {
	#[allow(dead_code)]
	SimpleMessage = 0,
	Simple,
	Os,
	Custom
}

impl Tag {
	const MASK: usize = 0b11;
}

pub struct Repr(NonNull<()>);

impl Repr {
	pub fn new(data: ErrorData<Custom>) -> Self {
		match data {
			ErrorData::Os(code) => Self::new_os(code),
			ErrorData::Simple(kind) => Self::new_simple(kind),
			ErrorData::SimpleMessage(msg) => Self::new_simple_message(msg),
			ErrorData::Custom(custom) => Self::new_custom(custom)
		}
	}

	pub const fn new_os(code: OsError) -> Self {
		const_assert!(Tag::Os as usize != 0);

		#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
		let tagged = (code as usize) << 8 | Tag::Os as usize;

		/* Safety: `Tag::Os` is non zero */
		Self(unsafe { NonNull::from_addr_unchecked(tagged) })
	}

	pub const fn new_simple(kind: ErrorKind) -> Self {
		const_assert!(Tag::Simple as usize != 0);

		let tagged = (kind as usize) << 8 | Tag::Simple as usize;

		/* Safety: `Tag::Simple` is non zero */
		Self(unsafe { NonNull::from_addr_unchecked(tagged) })
	}

	pub fn new_simple_message(msg: &'static SimpleMessage) -> Self {
		const_assert!(Tag::SimpleMessage as usize == 0);

		Self(ptr!(!null msg).cast())
	}

	pub fn new_custom(custom: Custom) -> Self {
		let tagged = custom.0.addr() | Tag::Custom as usize;

		forget(custom);

		/* Safety: custom has a non-null addr */
		Self(unsafe { NonNull::from_addr_unchecked(tagged) })
	}

	fn decode_repr<F, C>(repr: NonNull<()>, custom: F) -> ErrorData<C>
	where
		F: FnOnce(MutNonNull<DynError<()>>) -> C
	{
		let bits = repr.addr();

		/* Safety: 0..4 are valid tags */
		let tag = unsafe { transmute(bits & Tag::MASK) };
		let bits = bits & !Tag::MASK;

		/* Safety: we created these earlier */
		#[allow(
			clippy::multiple_unsafe_ops_per_block,
			clippy::cast_possible_truncation
		)]
		unsafe {
			match tag {
				#[cfg(feature = "os")]
				Tag::Os => ErrorData::Os(transmute((bits >> 8) as u16)),

				#[cfg(not(feature = "os"))]
				#[allow(clippy::cast_possible_wrap)]
				Tag::Os => ErrorData::Os((bits >> 8) as i32),

				/* this won't compile unless they're the same size */
				Tag::Simple => ErrorData::Simple(transmute((bits >> 8) as u8)),

				/* the tag is zero for this */
				Tag::SimpleMessage => ErrorData::SimpleMessage(repr.cast().as_ref()),
				Tag::Custom => {
					let ptr = MutNonNull::from_addr_unchecked(bits);

					ErrorData::Custom(custom(ptr))
				}
			}
		}
	}

	pub fn data(&self) -> ErrorData<CustomRef<'_>> {
		/* Safety: ref */
		Self::decode_repr(self.0, |ptr| unsafe { CustomRef::from(ptr) })
	}

	#[allow(clippy::needless_pass_by_ref_mut)]
	pub fn data_mut(&mut self) -> ErrorData<CustomRef<'_, true>> {
		/* Safety: ref */
		Self::decode_repr(self.0, |ptr| unsafe { CustomRef::from(ptr) })
	}

	#[must_use]
	pub fn into_data(self) -> ErrorData<Custom> {
		let ptr = self.0;

		forget(self);

		Self::decode_repr(ptr, Custom)
	}
}

impl From<Custom> for Repr {
	fn from(custom: Custom) -> Self {
		Self::new_custom(custom)
	}
}

impl Debug for Repr {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		match self.data() {
			ErrorData::Os(os) => Debug::fmt(&os, fmt),
			ErrorData::Simple(simple) => fmt.debug_tuple("Kind").field(&simple).finish(),
			ErrorData::SimpleMessage(msg) => Debug::fmt(msg, fmt),
			ErrorData::Custom(custom) => Debug::fmt(&custom, fmt)
		}
	}
}

impl Display for Repr {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		match self.data() {
			ErrorData::Os(os) => Display::fmt(&os, fmt),
			ErrorData::Simple(simple) => write!(fmt, "{}", simple),
			ErrorData::SimpleMessage(msg) => Display::fmt(msg, fmt),
			ErrorData::Custom(custom) => Display::fmt(&custom, fmt)
		}
	}
}

impl Drop for Repr {
	fn drop(&mut self) {
		Self::decode_repr(self.0, Custom);
	}
}

/* Safety: all constructors require the inner type to be Send */
unsafe impl Send for Repr {}

/* Safety: all constructors require the inner type to be Sync */
unsafe impl Sync for Repr {}
