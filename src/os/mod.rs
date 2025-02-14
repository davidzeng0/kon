use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::mem::{size_of, size_of_val, transmute};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::path::Path;
use std::time::Duration;

use enumflags2::{bitflags, make_bitflags, BitFlag, BitFlags};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use self::syscall::*;
use crate::error::*;
use crate::io::UninitBuf;
use crate::macros::syscall_define;
use crate::pointer::*;

pub mod dirent;
pub mod epoll;
pub mod error;
pub mod eventfd;
pub mod fcntl;
pub mod futex;
pub mod inet;
pub mod io_uring;
pub mod iovec;
pub mod mman;
pub mod openat;
pub mod openat2;
pub mod poll;
pub mod resource;
pub mod signal;
pub mod socket;
pub mod stat;
pub mod syscall;
pub mod tcp;
pub mod time;
pub mod unistd;

pub const INVALID_FD: RawFd = -1;

pub mod raw {
	use super::*;

	define_struct! {
		pub struct RawBuf {
			pub ptr: MutPtr<()>,
			pub len: usize
		}
	}

	#[repr(transparent)]
	#[derive(Default, Debug)]
	pub struct BorrowedRawBuf<'buf, const MUT: bool> {
		pub buf: RawBuf,
		pub phantom: PhantomData<&'buf ()>
	}
}

pub type RawBuf<'buf> = raw::BorrowedRawBuf<'buf, false>;
pub type MutRawBuf<'buf> = raw::BorrowedRawBuf<'buf, true>;

impl IntoRawArray for RawBuf<'_> {
	type Length = usize;
	type Pointer = Ptr<()>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		(self.buf.ptr.cast_const(), self.buf.len)
	}
}

impl IntoRawArray for MutRawBuf<'_> {
	type Length = usize;
	type Pointer = MutPtr<()>;

	fn into_raw_array(self) -> (Self::Pointer, Self::Length) {
		(self.buf.ptr, self.buf.len)
	}
}

impl RawBuf<'_> {
	#[must_use]
	pub const fn from_parts(value: Ptr<()>, len: usize) -> Self {
		Self {
			buf: raw::RawBuf { ptr: value.cast_mut(), len },
			phantom: PhantomData
		}
	}
}

impl<'buf> RawBuf<'buf> {
	#[must_use]
	pub const fn cast_mut(self) -> MutRawBuf<'buf> {
		MutRawBuf { buf: self.buf, phantom: self.phantom }
	}
}

impl MutRawBuf<'_> {
	#[must_use]
	pub const fn from_parts(value: MutPtr<()>, len: usize) -> Self {
		Self {
			buf: raw::RawBuf { ptr: value, len },
			phantom: PhantomData
		}
	}
}

impl<'buf, T> From<&'buf [T]> for RawBuf<'buf> {
	fn from(value: &'buf [T]) -> Self {
		Self {
			buf: raw::RawBuf {
				ptr: ptr!(value.as_ptr()).cast_mut().cast(),
				len: size_of_val(value)
			},
			phantom: PhantomData
		}
	}
}

impl<'buf, T> From<&'buf T> for RawBuf<'buf> {
	fn from(value: &'buf T) -> Self {
		Self {
			buf: raw::RawBuf {
				ptr: ptr!(value).cast_mut().cast(),
				len: size_of::<T>()
			},
			phantom: PhantomData
		}
	}
}

impl<'buf, T> From<&'buf mut [T]> for MutRawBuf<'buf> {
	fn from(value: &'buf mut [T]) -> Self {
		Self {
			buf: raw::RawBuf {
				ptr: ptr!(value.as_mut_ptr()).cast(),
				len: size_of_val(value)
			},
			phantom: PhantomData
		}
	}
}

impl<'buf, T> From<&'buf mut T> for MutRawBuf<'buf> {
	fn from(value: &'buf mut T) -> Self {
		Self {
			buf: raw::RawBuf { ptr: ptr!(value).cast(), len: size_of::<T>() },
			phantom: PhantomData
		}
	}
}

macro_rules! define_into_raw_repr {
	($name: ident #[repr($repr:ty)] $(#$rest:tt)*) => {
		impl IntoRaw for $name {
			type Raw = $repr;

			fn into_raw(self) -> $repr {
				self as $repr
			}
		}

		impl From<$name> for $repr {
			fn from(value: $name) -> Self {
				value as Self
			}
		}

		define_into_raw_repr!($name $(#$rest)*);
	};

	($name: ident #$attr:tt $(#$rest:tt)*) => {
		define_into_raw_repr!($name $(#$rest)*);
	};

	($name: ident) => {};
}

use define_into_raw_repr;

macro_rules! define_enum {
	(
		$(#$attrs: tt)*
		$vis: vis
		enum $name: ident
		$($rest: tt)*
	) => {
		#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, ::num_derive::FromPrimitive)]
		$(#$attrs)*
		$vis enum $name $($rest)*

		define_into_raw_repr!($name $(#$attrs)*);
	}
}

use define_enum;

macro_rules! define_struct {
	(
		$(#$attrs: tt)*
		$vis: vis
		struct $name: ident $(<$generic:ident: ?Sized>)?
		{ $($rest: tt)* }
	) => {
		#[derive(Clone, Copy, PartialEq, Eq, Debug)]
		#[repr(C)]
		$(#$attrs)*
		$vis struct $name $(<$generic: ?Sized>)?
		{ $($rest)* }

		#[allow(deprecated)]
		impl $(<$generic>)? ::std::default::Default for $name $(<$generic>)? {
			fn default() -> Self {
				/* Safety: repr(C) */
				unsafe { ::std::mem::zeroed() }
			}
		}
	};
}

use define_struct;

macro_rules! define_union {
	(
		$(#$attrs: tt)*
		$vis: vis
		union $name: ident
		$($rest: tt)*
	) => {
		#[derive(Clone, Copy, Eq)]
		#[repr(C)]
		$(#$attrs)*
		$vis union $name $($rest)*

		#[allow(deprecated)]
		impl ::std::default::Default for $name {
			fn default() -> Self {
				/* Safety: repr(C) */
				unsafe { ::std::mem::zeroed() }
			}
		}

		#[allow(deprecated)]
		impl ::std::cmp::PartialEq for $name {
			fn eq(&self, other: &Self) -> bool {
				::std::ptr::eq(self, other)
			}
		}

		#[allow(deprecated)]
		impl ::std::fmt::Debug for $name {
			fn fmt(&self, fmt: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
				fmt.debug_struct(::std::stringify!($name)).finish()
			}
		}
	}
}

use define_union;

#[cold]
#[inline(never)]
fn allocate_cstr<F, Output>(bytes: &[u8], func: F) -> Result<Output>
where
	F: FnOnce(&CStr) -> Result<Output>
{
	let str = CString::new(bytes)?;

	func(&str)
}

#[allow(clippy::impl_trait_in_params, clippy::multiple_unsafe_ops_per_block)]
pub fn with_path_as_cstr<F, Output>(path: impl AsRef<Path>, func: F) -> Result<Output>
where
	F: FnOnce(&CStr) -> Result<Output>
{
	const MAX_STACK_ALLOCATION: usize = 384;

	let bytes = path.as_ref().as_os_str().as_encoded_bytes();

	if bytes.len() >= MAX_STACK_ALLOCATION {
		allocate_cstr(bytes, func)
	} else {
		let mut buf = UninitBuf::<MAX_STACK_ALLOCATION>::new();

		buf.extend_from_slice(bytes);
		buf.extend_from_slice(&[0]);

		let str = CStr::from_bytes_with_nul(&buf)?;

		func(str)
	}
}
