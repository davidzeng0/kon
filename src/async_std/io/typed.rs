//! Traits, helpers, and type definitions for reading and writing typed data to
//! and from I/O

use std::fmt::{self, Arguments};
use std::mem::size_of;
use std::ops::BitAnd;

use super::*;
use crate::io::typed::*;
pub use crate::io::typed::{FromBytes, IntoBytes};
use crate::macros::{macro_each, paste};

#[asynchronous]
async fn read_bytes<R>(reader: &mut R, bytes: &mut [u8]) -> Result<usize>
where
	R: Read + ?Sized
{
	let read = reader.try_read_fully(bytes).await?;

	length_check(bytes, read);

	if read != 0 && read < bytes.len() {
		return Err(short_io_error_unless_interrupt().await);
	}

	Ok(read)
}

#[asynchronous]
async fn read_bytes_n<R, const N: usize>(reader: &mut R) -> Result<Option<[u8; N]>>
where
	R: Read + ?Sized
{
	let mut bytes = [0u8; N];
	let success = read_bytes(reader, &mut bytes).await? != 0;

	Ok(success.then_some(bytes))
}

#[asynchronous]
#[cold]
async fn buf_get_bytes_cold<R>(reader: &mut R, bytes: &mut [u8]) -> Result<usize>
where
	R: Read + ?Sized
{
	read_bytes(reader, bytes).await
}

#[asynchronous]
#[inline(always)]
#[allow(clippy::branches_sharing_code)]
async fn buf_get_bytes<R, const N: usize>(reader: &mut R, consume: usize) -> Result<Option<[u8; N]>>
where
	R: BufRead + ?Sized
{
	let available = reader.buffer();

	/* bytes variable is separated to improve optimizations */
	Ok(if available.len() >= N {
		let mut bytes = [0u8; N];

		/* this gets optimized to a single load instruction of size N, when N is a
		 * power of two */
		read_into_slice(&mut bytes, &available[0..N]);

		reader.consume(consume);

		Some(bytes)
	} else {
		let mut bytes = [0u8; N];
		let success = buf_get_bytes_cold(reader, &mut bytes[..consume]).await? != 0;

		success.then_some(bytes)
	})
}

#[asynchronous]
async fn buf_read_bytes<R, const N: usize>(reader: &mut R) -> Result<Option<[u8; N]>>
where
	R: BufRead + ?Sized
{
	buf_get_bytes(reader, N).await
}

trait VInt<const N: usize>: BitAnd<Self, Output = Self> + Sized {
	const MAX: Self;
	const ZERO: Self;

	fn from_le_bytes(bytes: [u8; N]) -> Self;
	fn from_be_bytes(bytes: [u8; N]) -> Self;
	fn wrapping_shr(self, shift: u32) -> Self;
}

macro_rules! impl_vint {
	($type:ty) => {
		impl VInt<{ size_of::<$type>() }> for $type {
			const MAX: Self = <$type>::MAX;
			const ZERO: Self = 0;

			fn from_le_bytes(bytes: [u8; size_of::<$type>()]) -> Self {
				Self::from_le_bytes(bytes)
			}

			fn from_be_bytes(bytes: [u8; size_of::<$type>()]) -> Self {
				Self::from_be_bytes(bytes)
			}

			fn wrapping_shr(self, shift: u32) -> Self {
				self.wrapping_shr(shift)
			}
		}
	};
}

macro_each!(impl_vint, u16, u32, u64, u128);

#[inline(always)]
fn vint_from_bytes<T, const N: usize>(bytes: [u8; N], size: usize, le: bool) -> T
where
	T: VInt<N>
{
	#[allow(clippy::cast_possible_truncation, clippy::arithmetic_side_effects)]
	let shift = (N - size) as u32 * 8;

	if le {
		let value = T::from_le_bytes(bytes);
		let mask = T::MAX.wrapping_shr(shift);

		/* LE 0ABC: CBAX -> XABC -> zero high bits */
		value & mask
	} else {
		let value = T::from_be_bytes(bytes);

		/* BE 0ABC: ABCX -> ABCX -> zero low bits */
		value.wrapping_shr(shift)
	}
}

#[asynchronous]
#[inline(always)]
async fn read_vint_fast<R, T, const N: usize>(
	reader: &mut R, size: usize, le: bool
) -> Result<Option<T>>
where
	R: BufRead + ?Sized,
	T: VInt<N>
{
	if size == 0 {
		return Ok(Some(T::ZERO));
	}

	assert!(size <= N, "Invalid size ({}) for variably sized int", size);

	buf_get_bytes(reader, size)
		.await
		.map(|c| c.map(|b| vint_from_bytes(b, size, le)))
}

#[asynchronous]
async fn read_vint<R, T, const N: usize>(reader: &mut R, size: usize, le: bool) -> Result<Option<T>>
where
	R: Read + ?Sized,
	T: VInt<N>
{
	if size == 0 {
		return Ok(Some(T::ZERO));
	}

	assert!(size <= N, "Invalid size ({}) for variably sized int", size);

	let mut bytes = [0u8; N];
	let success = read_bytes(reader, &mut bytes[..size]).await? != 0;

	Ok(success.then(|| vint_from_bytes(bytes, size, le)))
}

macro_rules! read_vint_type {
	($type:ty, $func:ident) => {
		paste! {
			#[doc = concat!(
				"Read a variable length int up to `",
				stringify!($type),
				"::MAX` from the stream in little endian order",
				"returning `None` if EOF and no bytes were read"
			)]
			/// # Cancel safety
			///
			/// This function is not cancel safe. Partial reads will lead to data loss
			#[inline]
			#[asynchronous(traitext)]
			async fn [< try_read_vint_ $type _le >](&mut self, size: usize) -> Result<Option<$type>> {
				[< $func >](self, size, true).await
			}

			#[doc = concat!(
				"Read a variable length int up to `",
				stringify!($type),
				"::MAX` from the stream in big endian order",
				"returning `None` if EOF and no bytes were read"
			)]
			/// # Cancel safety
			///
			/// This function is not cancel safe. Partial reads will lead to data loss
			#[inline]
			#[asynchronous(traitext)]
			async fn [< try_read_vint_ $type _be >](&mut self, size: usize) -> Result<Option<$type>> {
				[< $func >](self, size, false).await
			}

			#[doc = concat!(
				"Read a variable length int up to `",
				stringify!($type),
				"::MAX` from the stream in little endian order",
				"returning an error on EOF"
			)]
			/// # Cancel safety
			///
			/// This function is not cancel safe. Partial reads will lead to data loss
			#[inline]
			#[asynchronous(traitext)]
			async fn [< read_vint_ $type _le >](&mut self, size: usize) -> Result<$type> {
				[< $func >](self, size, true).await?.ok_or_else(|| ErrorKind::UnexpectedEof.into())
			}

			#[doc = concat!(
				"Read a variable length int up to `",
				stringify!($type),
				"::MAX` from the stream in big endian order",
				"returning an error on EOF"
			)]
			/// # Cancel safety
			///
			/// This function is not cancel safe. Partial reads will lead to data loss
			#[inline]
			#[asynchronous(traitext)]
			async fn [< read_vint_ $type _be >](&mut self, size: usize) -> Result<$type> {
				[< $func >](self, size, false).await?.ok_or_else(|| ErrorKind::UnexpectedEof.into())
			}
		}
	};
}

macro_rules! read_vint_impl {
	($func:ident) => {
		read_vint_type!(u16, $func);
		read_vint_type!(u32, $func);
		read_vint_type!(u64, $func);
		read_vint_type!(u128, $func);
	};
}

macro_rules! read_vfloat_impl {
	() => {
		/// Read an `f32` or `f64` from the stream in little endian order, returning
		/// `None` if EOF and no bytes were read
		///
		/// # Cancel safety
		///
		/// This function is not cancel safe. Partial reads will lead to data loss
		#[inline]
		#[asynchronous(traitext)]
		async fn try_read_vfloat_le(&mut self, size: usize) -> Result<Option<f64>> {
			if size == size_of::<f32>() {
				self.try_read_f32_le().await.map(|c| c.map(|v| v as f64))
			} else if size == size_of::<f64>() {
				self.try_read_f64_le().await
			} else {
				panic!("Invalid size ({}) for variably sized float", size);
			}
		}

		/// Read an `f32` or `f64` from the stream in big endian order, returning
		/// `None` if EOF and no bytes were read
		///
		/// # Cancel safety
		///
		/// This function is not cancel safe. Partial reads will lead to data loss
		#[inline]
		#[asynchronous(traitext)]
		async fn try_read_vfloat_be(&mut self, size: usize) -> Result<Option<f64>> {
			if size == size_of::<f32>() {
				self.try_read_f32_be().await.map(|c| c.map(|v| v as f64))
			} else if size == size_of::<f64>() {
				self.try_read_f64_be().await
			} else {
				panic!("Invalid size ({}) for variably sized float", size);
			}
		}

		/// Read an `f32` or `f64` from the stream in little endian order, returning an
		/// error on EOF
		///
		/// # Cancel safety
		///
		/// This function is not cancel safe. Partial reads will lead to data loss
		#[inline]
		#[asynchronous(traitext)]
		async fn read_vfloat_le(&mut self, size: usize) -> Result<f64> {
			self.try_read_vfloat_le(size)
				.await?
				.ok_or_else(|| ErrorKind::UnexpectedEof.into())
		}

		/// Read an `f32` or `f64` from the stream in big endian order, returning an
		/// error on EOF
		///
		/// # Cancel safety
		///
		/// This function is not cancel safe. Partial reads will lead to data loss
		#[inline]
		#[asynchronous(traitext)]
		async fn read_vfloat_be(&mut self, size: usize) -> Result<f64> {
			self.try_read_vfloat_be(size)
				.await?
				.ok_or_else(|| ErrorKind::UnexpectedEof.into())
		}
	};
}

macro_rules! read_num_type_endian {
	($type: ty, $endian_type: ty, $endian: ident, $endian_doc: literal) => {
		paste! {
			#[doc = concat!(
				"Read a [`",
				stringify!($type),
				"`] from the stream",
				$endian_doc,
				", returning `None` if EOF and no bytes were read"
			)]
			/// # Cancel Safety
			///
			/// This function is not cancel safe. Partial reads will lead to data loss
			#[asynchronous(traitext)]
			#[inline]
			async fn [<try_read_ $endian_type>](&mut self) -> Result<Option<$type>> {
				self.try_read_type::<[<$type $endian>], { [<$type $endian>]::BYTES }>().await.map(|c| c.map(|t| t.0))
			}

			#[doc = concat!(
				"Read a [`",
				stringify!($type),
				"`] from the stream",
				$endian_doc,
				", returning an error on EOF"
			)]
			/// # Cancel safety
			///
			/// This function is not cancel safe. Partial reads will lead to data loss
			#[asynchronous(traitext)]
			#[inline]
			async fn [<read_ $endian_type>](&mut self) -> Result<$type> {
				self.[<try_read_ $endian_type>]().await?.ok_or_else(|| ErrorKind::UnexpectedEof.into())
			}
		}
	};
}

macro_rules! read_num_type {
	($type:ty) => {
		paste! {
			read_num_type_endian!($type, [<$type _le>], le, " in little endian order");
			read_num_type_endian!($type, [<$type _be>], be, " in big endian order");
		}
	};
}

macro_rules! read_int {
	($bits:literal) => {
		paste! {
			read_num_type!([<i $bits>]);
			read_num_type!([<u $bits>]);
		}
	};
}

macro_rules! read_num_impl {
	($vint_func:ident) => {
		read_num_type_endian!(i8, i8, le, "");
		read_num_type_endian!(u8, u8, le, "");
		macro_each!(read_int, 16, 32, 64, 128);
		macro_each!(read_num_type, f32, f64);
		read_vfloat_impl!();
		read_vint_impl!($vint_func);
	};
}

/// Extension trait for reading typed data from a stream
pub trait ReadTyped: ReadSealed {
	read_num_impl!(read_vint);

	/// Read a type, returning `None` if EOF and no bytes were read
	///
	/// # Cancel safety
	///
	/// This function is not cancel safe. Partial reads will lead to data loss
	#[asynchronous(traitext)]
	async fn try_read_type<T, const N: usize>(&mut self) -> Result<Option<T>>
	where
		T: FromBytes<N>
	{
		read_bytes_n(self).await.map(|c| c.map(T::from_bytes))
	}

	/// Read a type, returning an error on EOF
	///
	/// # Cancel safety
	///
	/// This function is not cancel safe. Partial reads will lead to data loss
	#[asynchronous(traitext)]
	async fn read_type<T, const N: usize>(&mut self) -> Result<T>
	where
		T: FromBytes<N>
	{
		self.try_read_type()
			.await?
			.ok_or_else(|| ErrorKind::UnexpectedEof.into())
	}
}

impl<T: Read> ReadTyped for T {}

/// Extension trait for reading typed data from a stream. A [`BufRead`]
/// implementation allows for more efficient reads of small types
pub trait BufReadTyped: BufReadSealed {
	read_num_impl!(read_vint_fast);

	/// Read a type, returning `None` if EOF and no bytes were read
	///
	/// # Cancel safety
	///
	/// This function is not cancel safe. Partial reads will lead to data loss
	#[asynchronous(traitext)]
	async fn try_read_type<T, const N: usize>(&mut self) -> Result<Option<T>>
	where
		T: FromBytes<N>
	{
		buf_read_bytes(self).await.map(|c| c.map(T::from_bytes))
	}

	/// Read a type, returning an error on EOF
	///
	/// # Cancel safety
	///
	/// This function is not cancel safe. Partial reads will lead to data loss
	#[asynchronous(traitext)]
	async fn read_type<T, const N: usize>(&mut self) -> Result<T>
	where
		T: FromBytes<N>
	{
		self.try_read_type()
			.await?
			.ok_or_else(|| ErrorKind::UnexpectedEof.into())
	}
}

impl<T: BufRead> BufReadTyped for T {}

struct FmtAdapter<'a, W: Write> {
	writer: &'a mut W,
	context: &'a Context,
	wrote: usize,
	error: Option<Error>
}

#[asynchronous]
impl<'a, W: Write> FmtAdapter<'a, W> {
	fn new(writer: &'a mut W, context: &'a Context) -> Self {
		Self { writer, context, wrote: 0, error: None }
	}

	/// # Safety
	/// See [`scoped`]
	///
	/// [`scoped`]: crate::coroutines::scoped
	#[allow(unsafe_code)]
	unsafe fn write_args(&mut self, args: Arguments<'_>) -> Result<usize> {
		match fmt::write(self, args) {
			Ok(()) => Ok(self.wrote),
			Err(_) => Err(self
				.error
				.take()
				.unwrap_or_else(|| ErrorKind::FormatterError.into()))
		}
	}
}

impl<W: Write> fmt::Write for FmtAdapter<'_, W> {
	fn write_str(&mut self, s: &str) -> fmt::Result {
		/* Safety: guaranteed by caller of `write_args` */
		#[allow(unsafe_code)]
		let result = unsafe { scoped(self.context, self.writer.write_string(s)) };

		match result {
			Ok(n) => {
				/* we don't really care if it overflows */
				#[allow(clippy::arithmetic_side_effects)]
				(self.wrote += n);

				Ok(())
			}

			Err(err) => {
				self.error = Some(err);

				Err(fmt::Error)
			}
		}
	}
}

macro_rules! write_num_type_endian {
	($type: ty, $endian_type: ident, $endian: ident, $endian_doc: literal) => {
		paste! {
			#[doc = concat!(
				"Write a [`",
				stringify!($type),
				"`] to the stream",
				$endian_doc
			)]
			///
			/// Returns the number of bytes written
			///
			/// # Cancel Safety
			///
			/// This function is cancel safe. Once the interrupt has been cleared,
			/// resume by writing the remaining bytes for the type
			#[asynchronous(traitext)]
			async fn [<write_ $endian_type>](&mut self, val: $type) -> Result<usize> {
				self.write_type::<[<$type $endian>], { [<$type $endian>]::BYTES }>([<$type $endian>](val)).await
			}
		}
	};
}

macro_rules! write_num_type {
	($type:ty) => {
		paste! {
			write_num_type_endian!($type, [<$type _le>], le, " in little endian order");
			write_num_type_endian!($type, [<$type _be>], be, " in big endian order");
		}
	};
}

macro_rules! write_int {
	($bits:literal) => {
		paste! {
			write_num_type!([<i $bits>]);
			write_num_type!([<u $bits>]);
		}
	};
}

/// Extension trait for writing typed data from a stream
pub trait WriteTyped: WriteSealed {
	write_num_type_endian!(i8, i8, le, "");
	write_num_type_endian!(u8, u8, le, "");
	macro_each!(write_int, 16, 32, 64, 128);
	macro_each!(write_num_type, f32, f64);

	/// Writes format arguments to the stream
	///
	/// Returns the number of bytes written, or error if the data could not be
	/// fully written
	///
	/// # Cancel safety
	///
	/// This function is not cancel safe.
	#[asynchronous(traitext)]
	async fn write_fmt(&mut self, args: Arguments<'_>) -> Result<usize>
	where
		Self: Sized
	{
		let mut adapter = FmtAdapter::new(self, get_context().await);

		/* Safety: we are in an async function */
		#[allow(unsafe_code)]
		(unsafe { adapter.write_args(args) })
	}

	/// Attempts to write the entire string, returning the number of bytes
	/// written which may be short if interrupted or eof
	///
	/// # Cancel safety
	///
	/// This function is cancel safe. Once the interrupt has been cleared,
	/// resume by writing the remaining bytes of the string
	#[asynchronous(traitext)]
	async fn try_write_string(&mut self, buf: &str) -> Result<usize> {
		self.try_write_all(buf.as_bytes()).await
	}

	/// Same as [`try_write_string`] but returns error on partial
	/// writes
	///
	/// This function is not cancel safe. Data is lost on interrupt, since an
	/// error is returned.
	///
	/// [`try_write_string`]: WriteTyped::try_write_string
	#[asynchronous(traitext)]
	async fn write_string(&mut self, buf: &str) -> Result<usize> {
		self.write_all(buf.as_bytes()).await
	}

	/// Attemps to write an entire char, returning error on partial writes
	///
	/// # Cancel safety
	///
	/// This function is cancel safe. Once the interrupt has been cleared,
	/// resume by writing the remaining bytes of the char
	#[asynchronous(traitext)]
	async fn write_char(&mut self, ch: char) -> Result<usize> {
		let mut buf = [0u8; 4];

		self.write_string(ch.encode_utf8(&mut buf)).await
	}

	/// Attempts to write an entire type, returning the number of bytes written
	/// which may be short if interrupted or eof
	///
	/// # Cancel safety
	///
	/// This function is not cancel safe. Data is lost on interrupt, since an
	/// error is returned.
	#[asynchronous(traitext)]
	async fn write_type<T, const N: usize>(&mut self, val: T) -> Result<usize>
	where
		T: IntoBytes<N>
	{
		self.try_write_all(&val.into_bytes()).await
	}
}

impl<T: Write> WriteTyped for T {}
