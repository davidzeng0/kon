use std::backtrace::Backtrace;
use std::ffi::{FromBytesWithNulError, FromVecWithNulError, NulError};
use std::fmt::{self, Debug, Display, Formatter};
use std::str::Utf8Error;
use std::string::FromUtf8Error;
use std::{error, io, result};

use crate::macros::sealed_trait;
#[cfg(feature = "os")]
use crate::os::error::OsError;
#[cfg(not(feature = "os"))]
pub type OsError = i32;

pub mod common;
mod kind;
mod repr;

type BoxedError = Box<dyn error::Error + Send + Sync + 'static>;

#[doc(inline)]
pub use kind::*;
pub use repr::SimpleMessage;

pub use crate::macros::errors;

pub type Result<T> = result::Result<T, Error>;
pub type OsResult<T> = result::Result<T, OsError>;

mod private {
	use super::*;

	pub trait ErrorBounds: Display + Debug + Send + Sync + 'static {}

	impl<T: Display + Debug + Send + Sync + ?Sized + 'static> ErrorBounds for T {}

	pub trait Context: Display + Send + Sync + 'static {}

	impl<T: Display + Send + Sync + 'static> Context for T {}
}

#[doc(hidden)]
pub mod internal {
	use super::*;

	pub trait ErrorImpl: error::Error + ErrorBounds {
		fn into_error(self) -> Error
		where
			Self: Sized
		{
			Error(Custom::new_error_impl(self, ()).into())
		}

		fn kind(&self) -> ErrorKind {
			ErrorKind::Other
		}
	}
}

use self::internal::*;
use self::private::*;
use self::repr::*;

sealed_trait!();

impl<T> Sealed for Result<T> {}

pub trait ErrorContext<T>: Sealed {
	fn context<C>(self, context: C) -> Result<T>
	where
		C: Context;

	fn with_context<C, F>(self, context: F) -> Result<T>
	where
		C: Context,
		F: FnOnce() -> C;
}

impl<T> ErrorContext<T> for Result<T> {
	fn context<C>(self, context: C) -> Self
	where
		C: Context
	{
		self.with_context(|| context)
	}

	fn with_context<C, F>(self, context: F) -> Self
	where
		C: Context,
		F: FnOnce() -> C
	{
		match self {
			Ok(ok) => Ok(ok),
			Err(err) => Err(err.context(context()))
		}
	}
}

pub struct Error(Repr);

impl Error {
	#[must_use]
	pub fn os_error(&self) -> Option<OsError> {
		match self.0.data() {
			ErrorData::Os(os) => Some(os),
			_ => None
		}
	}

	#[must_use]
	pub fn kind(&self) -> ErrorKind {
		match self.0.data() {
			ErrorData::Os(os) => os.into(),
			ErrorData::Simple(kind) => kind,
			ErrorData::SimpleMessage(msg) => msg.kind,
			ErrorData::Custom(custom) => custom.kind()
		}
	}

	#[must_use]
	pub fn is_interrupted(&self) -> bool {
		self.kind() == ErrorKind::Interrupted
	}

	#[must_use]
	pub fn message<E>(err: E) -> Self
	where
		E: ErrorBounds
	{
		Self(Custom::new_basic(err, ()).into())
	}

	#[must_use]
	pub fn new<E>(err: E) -> Self
	where
		E: ErrorBounds + error::Error
	{
		Self(Custom::new_std(err, ()).into())
	}

	pub fn context<C>(self, context: C) -> Self
	where
		C: Display + Send + Sync + 'static
	{
		Self(Custom::new_context(context, self).into())
	}

	#[must_use]
	pub fn backtrace(&self) -> Option<&Backtrace> {
		match self.0.data() {
			ErrorData::Custom(custom) => custom.backtrace(),
			_ => None
		}
	}

	#[must_use]
	pub fn downcast_ref<E>(&self) -> Option<&E>
	where
		E: ErrorBounds
	{
		match self.0.data() {
			ErrorData::Custom(custom) => custom.downcast_ref(),
			_ => None
		}
	}

	#[must_use]
	pub fn downcast_mut<E>(&mut self) -> Option<&mut E>
	where
		E: ErrorBounds
	{
		match self.0.data_mut() {
			ErrorData::Custom(mut custom) => custom.downcast_mut(),
			_ => None
		}
	}

	pub fn downcast<E>(self) -> Result<E>
	where
		E: ErrorBounds
	{
		match self.0.into_data() {
			ErrorData::Custom(custom) => custom.downcast().map_err(|custom| Self(custom.into())),
			data => Err(Self(Repr::new(data)))
		}
	}
}

impl Debug for Error {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Debug::fmt(&self.0, fmt)
	}
}

impl Display for Error {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		Display::fmt(&self.0, fmt)
	}
}

impl error::Error for Error {
	fn source(&self) -> Option<&(dyn error::Error + 'static)> {
		match self.0.data() {
			ErrorData::Custom(custom) => custom.source(),
			_ => None
		}
	}
}

impl From<ErrorKind> for Error {
	fn from(value: ErrorKind) -> Self {
		Self(Repr::new_simple(value))
	}
}

impl From<&'static SimpleMessage> for Error {
	fn from(value: &'static SimpleMessage) -> Self {
		Self(Repr::new_simple_message(value))
	}
}

impl From<BoxedError> for Error {
	fn from(value: BoxedError) -> Self {
		Self(Custom::new_boxed(value, ()).into())
	}
}

impl From<fmt::Arguments<'_>> for Error {
	fn from(value: fmt::Arguments<'_>) -> Self {
		match value.as_str() {
			Some(str) => Self::message(str),
			None => Self::message(fmt::format(value))
		}
	}
}

impl From<OsError> for Error {
	fn from(value: OsError) -> Self {
		Self(Repr::new_os(value))
	}
}

impl<T: ErrorImpl> From<T> for Error {
	fn from(value: T) -> Self {
		value.into_error()
	}
}

impl From<Utf8Error> for Error {
	fn from(_: Utf8Error) -> Self {
		common::INVALID_UTF8.into()
	}
}

impl From<FromUtf8Error> for Error {
	fn from(_: FromUtf8Error) -> Self {
		common::INVALID_UTF8.into()
	}
}

impl From<NulError> for Error {
	fn from(_: NulError) -> Self {
		common::INVALID_CSTR.into()
	}
}

impl From<FromBytesWithNulError> for Error {
	fn from(_: FromBytesWithNulError) -> Self {
		common::INVALID_CSTR.into()
	}
}

impl From<FromVecWithNulError> for Error {
	fn from(_: FromVecWithNulError) -> Self {
		common::INVALID_CSTR.into()
	}
}

impl<T: PartialEq + ErrorImpl> PartialEq<T> for Error {
	fn eq(&self, other: &T) -> bool {
		self.downcast_ref::<T>() == Some(other)
	}
}

impl PartialEq<OsError> for Error {
	fn eq(&self, other: &OsError) -> bool {
		self.os_error() == Some(*other)
	}
}

impl PartialEq<ErrorKind> for Error {
	fn eq(&self, other: &ErrorKind) -> bool {
		self.kind() == *other
	}
}

impl ErrorImpl for io::Error {
	fn into_error(self) -> Error
	where
		Self: Sized
	{
		if let Some(code) = self.raw_os_error() {
			#[cfg(feature = "os")]
			return OsError::from(code).into();
			#[cfg(not(feature = "os"))]
			return code.into();
		}

		if self.get_ref().is_some() {
			let kind = ErrorKind::from(self.kind());

			#[allow(clippy::unwrap_used)]
			let inner = self.into_inner().unwrap();

			return match (inner.downcast(), kind) {
				(Ok(err), _) => *err,
				(Err(boxed), ErrorKind::Other) => Error(Custom::new_boxed(boxed, ()).into()),
				(Err(boxed), kind) => Error(Custom::new_boxed(boxed, kind).into())
			};
		}

		Error(Custom::new_error_impl(self, ()).into())
	}

	fn kind(&self) -> ErrorKind {
		self.kind().into()
	}
}

impl From<Error> for io::Error {
	fn from(value: Error) -> Self {
		if let Some(os) = value.os_error() {
			#[allow(clippy::unnecessary_cast)]
			Self::from_raw_os_error(os as i32)
		} else {
			Self::new(io::ErrorKind::Other, value)
		}
	}
}

#[cfg(feature = "os")]
impl Debug for OsError {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		fmt.debug_struct("Os")
			.field("code", &(*self as i32))
			.field("kind", &ErrorKind::from(*self))
			.field("message", &self.as_str())
			.finish()
	}
}

#[cfg(feature = "os")]
impl Display for OsError {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		write!(fmt, "{} (os error {})", self.as_str(), *self as i32)
	}
}

#[macro_export]
macro_rules! fmt_error {
	($str:literal) => {
		$crate::error::fmt_error!($str @ $crate::error::ErrorKind::Other);
	};

	($str:literal @ $kind:expr) => {
		<$crate::error::Error as ::std::convert::From<
			&'static $crate::error::SimpleMessage
		>>::from(
			&$crate::error::SimpleMessage {
				kind: $kind,
				message: $str
			}
		)
	};

	($($arg:tt)*) => {
		<$crate::error::Error as ::std::convert::From<
			::std::fmt::Arguments<'_>
		>>::from(
			::std::format_args!($($arg)*)
		)
	}
}

pub use fmt_error;
