use crate::error::*;
use crate::macros::{import_sysdeps, macro_each, syscall_impl};

import_sysdeps!();

pub use platform::*;
pub use SyscallNumber::*;

use super::error::*;
use super::*;

#[derive(Clone, Copy)]
pub struct SyscallParameter(pub usize);

#[derive(Clone, Copy)]
pub struct SyscallResult(pub isize);

macro_rules! impl_from_primitive {
	($type:ty) => {
		impl From<$type> for SyscallParameter {
			fn from(value: $type) -> Self {
				#[allow(
					clippy::cast_possible_wrap,
					clippy::cast_sign_loss,
					clippy::cast_possible_truncation
				)]
				SyscallParameter(value as usize)
			}
		}
	};
}

macro_each!(
	impl_from_primitive,
	usize,
	isize,
	u32,
	i32,
	u16,
	i16,
	u8,
	i8
);

#[cfg(target_pointer_width = "64")]
macro_each!(impl_from_primitive, u64, i64);

impl<T: ?Sized, const MUT: bool> From<Pointer<T, MUT>> for SyscallParameter {
	fn from(value: Pointer<T, MUT>) -> Self {
		value.addr().into()
	}
}

macro_rules! impl_pointer {
	($type:ty) => {
		#[allow(unused_parens)]
		impl<T: ?Sized> From<$type> for SyscallParameter {
			fn from(value: $type) -> Self {
				Pointer::from(value).into()
			}
		}
	};
}

macro_each!(impl_pointer, (&T), (&mut T), (*const T), (*mut T));

impl From<SyscallResult> for OsResult<()> {
	fn from(val: SyscallResult) -> Self {
		result_from_int(val.0).map(|_| ())
	}
}

macro_rules! impl_primitive_from {
	($type:ty) => {
		impl From<SyscallResult> for OsResult<$type> {
			fn from(val: SyscallResult) -> Self {
				#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
				result_from_int(val.0).map(|result| result as $type)
			}
		}
	};
}

macro_each!(impl_primitive_from, u32, i32, u64, i64, usize, isize);

impl From<SyscallResult> for OsResult<OwnedFd> {
	fn from(val: SyscallResult) -> Self {
		result_from_int(val.0).map(|raw_fd| {
			/* Safety: guaranteed by syscall declaration */
			#[allow(clippy::cast_possible_truncation)]
			(unsafe { OwnedFd::from_raw_fd(raw_fd as i32) })
		})
	}
}

impl<T, const MUT: bool> From<SyscallResult> for OsResult<Pointer<T, MUT>> {
	fn from(val: SyscallResult) -> Self {
		result_from_ptr(val.0).map(|addr| Pointer::from_addr(addr))
	}
}

pub trait IntoRaw {
	type Raw: Into<SyscallParameter>;

	fn into_raw(self) -> Self::Raw;
}

impl<T: Into<SyscallParameter>> IntoRaw for T {
	type Raw = Self;

	fn into_raw(self) -> Self {
		self
	}
}

impl<T: BitFlag<Numeric = Numeric>, Numeric: Into<SyscallParameter>> IntoRaw for BitFlags<T> {
	type Raw = T::Numeric;

	fn into_raw(self) -> Self::Raw {
		self.bits()
	}
}

impl IntoRaw for OwnedFd {
	type Raw = RawFd;

	fn into_raw(self) -> i32 {
		self.into_raw_fd()
	}
}

impl IntoRaw for BorrowedFd<'_> {
	type Raw = RawFd;

	fn into_raw(self) -> i32 {
		self.as_raw_fd()
	}
}

impl IntoRaw for Option<BorrowedFd<'_>> {
	type Raw = RawFd;

	fn into_raw(self) -> i32 {
		self.as_ref().map_or(INVALID_FD, AsRawFd::as_raw_fd)
	}
}

impl<T> IntoRaw for Option<&T> {
	type Raw = Ptr<T>;

	fn into_raw(self) -> Ptr<T> {
		self.map_or(Ptr::null(), Ptr::from)
	}
}

impl<T> IntoRaw for Option<&mut T> {
	type Raw = MutPtr<T>;

	fn into_raw(self) -> MutPtr<T> {
		self.map_or(MutPtr::null(), MutPtr::from)
	}
}

pub trait IntoRawArray {
	type Pointer: IntoRaw;
	type Length: IntoRaw;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length);
}

impl<T> IntoRawArray for &[T] {
	type Length = usize;
	type Pointer = Ptr<T>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		(self.as_ptr().into(), self.len())
	}
}

impl<T> IntoRawArray for &mut [T] {
	type Length = usize;
	type Pointer = MutPtr<T>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		(self.as_mut_ptr().into(), self.len())
	}
}

impl<T> IntoRawArray for Option<&[T]> {
	type Length = usize;
	type Pointer = Ptr<T>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		self.map_or((Ptr::null(), 0), IntoRawArray::into_raw_array)
	}
}

impl<T> IntoRawArray for Option<&mut [T]> {
	type Length = usize;
	type Pointer = MutPtr<T>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		self.map_or((MutPtr::null(), 0), IntoRawArray::into_raw_array)
	}
}

#[macro_export]
macro_rules! syscall_raw {
	($num:expr) => {
		$crate::os::syscall::syscall0($num)
	};

	($num:expr, $arg1:expr) => {
		$crate::os::syscall::syscall1($num, $arg1)
	};

	($num:expr, $arg1:expr, $arg2:expr) => {
		$crate::os::syscall::syscall2($num, $arg1, $arg2)
	};

	($num:expr, $arg1:expr, $arg2:expr, $arg3:expr) => {
		$crate::os::syscall::syscall3($num, $arg1, $arg2, $arg3)
	};

	($num:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr) => {
		$crate::os::syscall::syscall4($num, $arg1, $arg2, $arg3, $arg4)
	};

	($num:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr) => {
		$crate::os::syscall::syscall5($num, $arg1, $arg2, $arg3, $arg4, $arg5)
	};

	($num:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr) => {
		$crate::os::syscall::syscall6($num, $arg1, $arg2, $arg3, $arg4, $arg5, $arg6)
	};
}

pub use syscall_raw;

#[macro_export]
macro_rules! syscall_int {
	($($arg: expr),+) => {
		$crate::os::error::result_from_int($crate::os::syscall::syscall_raw!($($arg),+))
	};
}

pub use syscall_int;

#[macro_export]
macro_rules! syscall_pointer {
	($($arg: expr),+) => {
		$crate::os::error::result_from_ptr($crate::os::syscall::syscall_raw!($($arg),+))
	};
}

pub use syscall_pointer;
