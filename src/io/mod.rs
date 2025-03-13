#![allow(clippy::transmute_ptr_to_ptr)]

use std::io::{IoSlice, Result, Write};
use std::mem::{transmute, MaybeUninit};
use std::ops::{Deref, DerefMut};

use crate::pointer::*;

pub mod typed;

/// Read from `src` to `dest`
pub fn read_into_slice(dest: &mut [u8], src: &[u8]) -> usize {
	let len = dest.len().min(src.len());

	/* adding any checks for small lengths only worsens performance
	 * it seems like llvm or rustc can't do branching properly
	 * (unlikely branches should be placed at the end, but that doesn't happen)
	 *
	 * a call to memcpy should do those checks anyway
	 */
	dest[0..len].copy_from_slice(&src[0..len]);
	len
}

pub fn read_into_uninit_slice<'dest>(
	dest: &'dest mut [MaybeUninit<u8>], src: &[u8]
) -> &'dest mut [u8] {
	let len = dest.len().min(src.len());

	/* Safety: len is in range */
	unsafe {
		ptr!(dest.as_mut_ptr())
			.cast::<u8>()
			.copy_from_nonoverlapping(ptr!(src.as_ptr()), len);
	}

	/* Safety: bytes are initalized */
	unsafe { transmute(&mut dest[0..len]) }
}

pub struct UninitBuf<const SIZE: usize> {
	data: [MaybeUninit<u8>; SIZE],
	filled: usize
}

impl<const SIZE: usize> UninitBuf<SIZE> {
	#[must_use]
	pub const fn new() -> Self {
		Self { data: [MaybeUninit::uninit(); SIZE], filled: 0 }
	}

	#[must_use]
	#[allow(clippy::arithmetic_side_effects)]
	pub const fn spare_capacity(&self) -> usize {
		self.data.len() - self.filled
	}

	pub fn extend_from_slice(&mut self, buf: &[u8]) -> usize {
		let len = read_into_uninit_slice(&mut self.data[self.filled..], buf).len();

		#[allow(clippy::arithmetic_side_effects)]
		(self.filled += len);

		len
	}

	#[must_use]
	pub fn data(&self) -> &[u8] {
		/* Safety: bytes are initialized */
		unsafe { transmute(&self.data[0..self.filled]) }
	}

	#[must_use]
	pub fn data_mut(&mut self) -> &mut [u8] {
		/* Safety: bytes are initialized */
		unsafe { transmute(&mut self.data[0..self.filled]) }
	}
}

impl<const SIZE: usize> Default for UninitBuf<SIZE> {
	fn default() -> Self {
		Self::new()
	}
}

impl<const SIZE: usize> Deref for UninitBuf<SIZE> {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		self.data()
	}
}

impl<const SIZE: usize> DerefMut for UninitBuf<SIZE> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.data_mut()
	}
}

impl<const SIZE: usize> Write for UninitBuf<SIZE> {
	fn write(&mut self, buf: &[u8]) -> Result<usize> {
		Ok(self.extend_from_slice(buf))
	}

	fn flush(&mut self) -> Result<()> {
		Ok(())
	}

	fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
		let mut total = 0;

		for buf in bufs {
			let wrote = self.extend_from_slice(buf);

			if wrote == 0 {
				break;
			}

			#[allow(clippy::arithmetic_side_effects)]
			(total += wrote);
		}

		Ok(total)
	}
}
