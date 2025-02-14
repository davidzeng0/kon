#![warn(unsafe_code)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The async equivalent of [`std::io`]

use std::io::{IoSlice, IoSliceMut, SeekFrom};
use std::mem::{take, transmute};
use std::str::from_utf8;

use super::*;
use crate::io::*;
use crate::macros::{macro_each, sealed_trait};
use crate::opt::hint::*;

macro_each!(sealed_trait, (trait Read), (trait BufRead), (trait Write));

pub mod buf_reader;
pub mod buf_writer;
pub mod read;
pub mod seek;
pub mod split;
pub mod typed;
pub mod write;

#[doc(inline)]
pub use {buf_reader::*, buf_writer::*, read::*, seek::*, split::*, write::*};

/// The default buffer size (16 KiB) for buffered I/O
pub const DEFAULT_BUFFER_SIZE: usize = 0x4000;

/// If `len` is zero, checks if the current async task is interrupted
///
/// This is useful for read and write functions where an interrupt cancelling
/// the operation may cause it to return zero. In this case, an error should be
/// returned instead of zero.
///
/// See also [`check_interrupt`]
#[asynchronous]
pub async fn check_interrupt_if_zero(len: usize) -> Result<usize> {
	if len == 0 {
		check_interrupt().await?;
	}

	Ok(len)
}

/// Advances a slice of slices
///
/// See also [`std::io::IoSlice::advance_slices`]
///
/// # Panics
/// If `amount` is greater than the length of the buffers combined
pub fn advance_slices(bufs: &mut &mut [IoSlice<'_>], mut amount: usize) {
	let mut remove = 0;

	for buf in bufs.iter_mut() {
		if let Some(amt) = amount.checked_sub(buf.len()) {
			amount = amt;

			#[allow(clippy::arithmetic_side_effects)]
			(remove += 1);
		} else {
			let left = &buf[amount..];

			#[allow(unsafe_code)]
			/* Safety: this mimics the IoSliceMut::advance function, no lifetimes are
			 * violated here
			 */
			(*buf = IoSlice::new(unsafe { transmute(left) }));
			amount = 0;

			break;
		}
	}

	*bufs = &mut take(bufs)[remove..];

	assert_eq!(amount, 0);
}

/// Advances a slice of slices
///
/// See also [`std::io::IoSlice::advance_slices`]
///
/// # Panics
/// If `amount` is greater than the length of the buffers combined
pub fn advance_slices_mut(bufs: &mut &mut [IoSliceMut<'_>], mut amount: usize) {
	let mut remove = 0;

	for buf in bufs.iter_mut() {
		if let Some(amt) = amount.checked_sub(buf.len()) {
			amount = amt;

			#[allow(clippy::arithmetic_side_effects)]
			(remove += 1);
		} else {
			let left = &mut buf[amount..];

			#[allow(unsafe_code)]
			/* Safety: this mimics the IoSliceMut::advance function, no lifetimes are
			 * violated here */
			(*buf = IoSliceMut::new(unsafe { transmute(left) }));
			amount = 0;

			break;
		}
	}

	*bufs = &mut take(bufs)[remove..];

	assert_eq!(amount, 0);
}

/// Checks that `len <= buf.len()`
///
/// Useful for callers of read/write functions to prevent overflow by ensuring
/// that the underlying implementation returns valid byte counts
///
/// Returns `len`
///
/// # Panics
/// if `len > buf.len()`
#[allow(clippy::must_use_candidate)]
pub fn length_check(buf: &[u8], len: usize) -> usize {
	assert!(len <= buf.len());

	len
}

/// If the current worker is interrupted, returns an [`Interrupted`] error.
/// Otherwise, returns an [`UnexpectedEof`] error
///
/// See also [`check_interrupt`]
///
/// [`UnexpectedEof`]: ErrorKind::UnexpectedEof
/// [`Interrupted`]: ErrorKind::Interrupted
#[asynchronous]
pub async fn short_io_error_unless_interrupt() -> Error {
	check_interrupt()
		.await
		.err()
		.unwrap_or_else(|| ErrorKind::UnexpectedEof.into())
}

/// Returns zero from the current function if the input buffer is empty. An
/// optional limit can be applied, in which case the buffer is truncated to that
/// length.
///
/// # Examples
///
/// ```
/// fn do_read(buf: &mut [u8]) -> Result<usize> {
/// 	read_into!(buf, 5);
///
/// 	assert!(matches!(buf.len(), 1..=5));
///
/// 	// do the read
/// }
/// ```
#[macro_export]
macro_rules! read_into {
	($buf:ident) => {
		if $crate::opt::hint::unlikely($buf.is_empty()) {
			return Ok(0);
		}
	};

	($buf:ident, $limit:expr) => {
		let $buf = {
			let min = $buf.len().min($limit);
			let buf = &mut $buf[0..min];

			read_into!(buf);

			buf
		};
	};
}

pub use read_into;

/// Returns zero from the current function if the input buffer is empty.
///
/// # Examples
///
/// ```
/// fn do_write(buf: &mut [u8]) -> Result<usize> {
/// 	write_from!(buf);
///
/// 	assert!(!buf.is_empty());
///
/// 	// do the write
/// }
/// ```
#[macro_export]
macro_rules! write_from {
	($buf:ident) => {
		if $crate::opt::hint::unlikely($buf.is_empty()) {
			return Ok(0);
		}
	};
}

pub use write_from;
