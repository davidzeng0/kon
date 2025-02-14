use std::io::{IoSlice, IoSliceMut};
use std::ops::{Deref, DerefMut};
use std::slice;

use super::*;

pub mod raw {
	use super::*;

	define_struct! {
		pub struct IoVec {
			pub base: MutPtr<()>,
			pub len: usize
		}
	}

	#[repr(transparent)]
	#[derive(Default, Debug)]
	pub struct BorrowedIoVec<'buf, const MUT: bool> {
		pub vec: IoVec,
		pub phantom: PhantomData<&'buf ()>
	}
}

pub type IoVec<'buf> = raw::BorrowedIoVec<'buf, false>;
pub type IoVecMut<'buf> = raw::BorrowedIoVec<'buf, true>;

impl<const MUT: bool> Deref for raw::BorrowedIoVec<'_, MUT> {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		/* Safety: we are borrowing the vec */
		unsafe { slice::from_raw_parts(self.vec.base.as_ptr().cast(), self.vec.len) }
	}
}

impl DerefMut for IoVecMut<'_> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		/* Safety: we are borrowing the vec */
		unsafe { slice::from_raw_parts_mut(self.vec.base.as_mut_ptr().cast(), self.vec.len) }
	}
}

impl<'buf> IoVec<'buf> {
	#[must_use]
	pub fn from_io_slices<'slices>(slices: &'slices [IoSlice<'buf>]) -> &'slices [Self] {
		/* Safety: they are the same */
		#[allow(clippy::transmute_ptr_to_ptr)]
		(unsafe { transmute(slices) })
	}
}

impl<'buf> IoVecMut<'buf> {
	#[must_use]
	pub fn from_io_slices_mut<'slices>(
		slices: &'slices mut [IoSliceMut<'buf>]
	) -> &'slices mut [Self] {
		/* Safety: they are the same */
		#[allow(clippy::transmute_ptr_to_ptr)]
		(unsafe { transmute(slices) })
	}
}

impl<'buf> From<IoVecMut<'buf>> for IoVec<'buf> {
	fn from(value: IoVecMut<'buf>) -> Self {
		Self { vec: value.vec, phantom: PhantomData }
	}
}

impl<'buf> From<&'buf [u8]> for IoVec<'buf> {
	fn from(value: &'buf [u8]) -> Self {
		Self {
			vec: raw::IoVec {
				base: ptr!(value.as_ptr()).cast_mut().cast(),
				len: value.len()
			},
			phantom: PhantomData
		}
	}
}

impl<'buf> From<&'buf mut [u8]> for IoVecMut<'buf> {
	fn from(value: &mut [u8]) -> Self {
		Self {
			vec: raw::IoVec {
				base: ptr!(value.as_mut_ptr()).cast(),
				len: value.len()
			},
			phantom: PhantomData
		}
	}
}
