//! Traits, helpers, and type definitions for reading from I/O

use memchr::memchr;

use super::*;
use crate::coroutines::ops::AsyncFnOnce;

/// Appends to `buf` by calling `read` with the string's buffer
///
/// The buffer's length is untouched if an unwind occurs or the additional bytes
/// are not valid utf-8
///
/// # Cancel safety
///
/// This function is not cancel safe. An interrupt may stop reading at a
/// character boundary, in which case the byte sequence would not be valid
/// utf-8. The buffer is then truncated leading to data loss.
#[asynchronous]
pub async fn append_to_string<F>(buf: &mut String, read: F) -> Result<Option<usize>>
where
	F: AsyncFnOnce(&mut Vec<u8>) -> Result<Option<usize>>
{
	/* panic guard */
	struct Guard<'a> {
		buf: &'a mut Vec<u8>,
		len: usize
	}

	impl Drop for Guard<'_> {
		fn drop(&mut self) {
			self.buf.truncate(self.len);
		}
	}

	#[allow(unsafe_code)]
	let mut guard = Guard {
		len: buf.len(),
		/* Safety: we truncate if the utf8 check fails */
		buf: unsafe { buf.as_mut_vec() }
	};

	let read = read.call_once(guard.buf).await?;

	if read.is_some() {
		from_utf8(&guard.buf[guard.len..])?;

		guard.len = guard.buf.len();
	}

	Ok(read)
}

#[asynchronous]
async fn default_read_vectored<R>(
	reader: &mut R, mut bufs: &mut [IoSliceMut<'_>]
) -> Result<(usize, bool)>
where
	R: Read + ?Sized
{
	let mut total = 0;

	while !bufs.is_empty() {
		let read = match reader.read_vectored(bufs).await {
			Ok(0) => break,
			Ok(n) => n,
			Err(err) if err.is_interrupted() => break,
			Err(err) => return Err(err)
		};

		advance_slices_mut(&mut bufs, read);

		/* checked by `advance_slices_mut` */
		#[allow(clippy::arithmetic_side_effects)]
		(total += read);
	}

	Ok((total, bufs.is_empty()))
}

/// The async equivalent of [`std::io::Read`]
#[asynchronous(impl(mut, box))]
pub trait Read {
	/// Read into `buf`, returning the amount of bytes read
	///
	/// Returns zero if the `buf` is empty, or if the stream reached EOF
	///
	/// See also [`std::io::Read::read`]
	///
	/// # Cancel safety
	///
	/// This function is usually cancel safe, however that depends on the exact
	/// implementation. Once the interrupt is cleared, call this function again
	/// to resume the operation.
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

	/// Read until the buffer is filled, an I/O error, an interrupt, or EOF
	///
	/// On interrupted, returns the number of bytes read if it is not zero
	///
	/// See also [`std::io::Read::read_exact`]
	///
	/// # Cancel safety
	///
	/// This function is usually cancel safe, however that depends on the exact
	/// implementation. Once the interrupt is cleared, call this function again
	/// to resume the operation.
	async fn try_read_fully(&mut self, buf: &mut [u8]) -> Result<usize> {
		read_into!(buf);

		let mut read = 0;

		while read < buf.len() {
			let available = &mut buf[read..];

			#[allow(clippy::arithmetic_side_effects)]
			match self.read(available).await {
				Ok(0) => break,
				Ok(n) => read += length_check(available, n),
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}
		}

		check_interrupt_if_zero(read).await
	}

	/// Same as [`try_read_fully`], except returns an [`UnexpectedEof`] error
	/// on partial reads
	///
	/// Returns the number of bytes read, which should be the same as
	/// `buf.len()`
	///
	/// See also [`std::io::Read::read_exact`]
	///
	/// # Cancel safety
	///
	/// This function is not cancel safe. Data is lost on interrupt, since an
	/// error is returned.
	///
	/// [`try_read_fully`]: Read::try_read_fully
	/// [`UnexpectedEof`]: ErrorKind::UnexpectedEof
	async fn read_fully(&mut self, buf: &mut [u8]) -> Result<usize> {
		read_into!(buf);

		let read = self.try_read_fully(buf).await?;

		length_check(buf, read);

		if unlikely(read != buf.len()) {
			return Err(short_io_error_unless_interrupt().await);
		}

		Ok(read)
	}

	/// Reads until EOF, an I/O error, or an interrupt
	///
	/// On interrupted, returns the number of bytes read if it is not zero
	///
	/// See also [`std::io::Read::read_to_end`]
	///
	/// # Cancel safety
	///
	/// This function is cancel safe. Once the interrupt is cleared, call this
	/// function again to resume the operation.
	async fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
		let start_len = buf.len();
		let mut len = buf.len();

		loop {
			if is_interrupted().await {
				/* avoid doubling the capacity if we're interrupted */
				break;
			}

			let mut capacity = buf.capacity();

			if len == capacity {
				buf.reserve(32);
				capacity = buf.capacity();
			}

			if buf.len() < capacity {
				buf.resize(capacity, 0);
			}

			let available = &mut buf[len..];

			#[allow(clippy::arithmetic_side_effects)]
			match self.read(available).await {
				Ok(0) => break,
				Ok(n) => len += length_check(available, n),
				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}

			if len < capacity {
				continue;
			}

			/* avoid doubling the capacity if EOF. try probing for more data */
			let mut probe = [0u8; 32];

			match self.read(&mut probe).await {
				Ok(0) => break,
				Ok(n) => {
					buf.extend_from_slice(&probe[0..n]);
					len = buf.len();
				}

				Err(err) if err.is_interrupted() => break,
				Err(err) => return Err(err)
			}
		}

		buf.truncate(len);

		#[allow(clippy::arithmetic_side_effects)]
		check_interrupt_if_zero(len - start_len).await
	}

	/// Reads until EOF, an I/O error, or an interrupt
	///
	/// On interrupt, returns the number of bytes read if it is not zero
	///
	/// See also [`std::io::Read::read_to_string`]
	///
	/// # Cancel safety
	///
	/// This function is not cancel safe. Interrupts may cause the read
	/// operation to stop in the middle of a character, in which case an
	/// invalid utf-8 error may be returned and the buffer truncated to its
	/// original length.
	async fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
		append_to_string(buf, |vec: &mut Vec<u8>| async move {
			self.read_to_end(vec).await.map(Some)
		})
		.await
		.map(Option::unwrap)
	}

	/// Returns `true` if this `Read` implementation has an efficient
	/// [`read_vectored`] implementation
	///
	/// See also [`std::io::Read::is_read_vectored`]
	///
	/// [`read_vectored`]: Read::read_vectored
	fn is_read_vectored(&self) -> bool {
		false
	}

	/// Like [`read`], except that it reads into a slice of buffers
	///
	/// See also [`std::io::Read::read_vectored`]
	///
	/// # Cancel safety
	///
	/// This function is usually cancel safe, however that depends on the exact
	/// implementation. Once the interrupt is cleared, call this function again
	/// to resume the operation.
	///
	/// [`read`]: Read::read
	async fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
		match bufs.iter_mut().find(|b| !b.is_empty()) {
			Some(buf) => self.read(&mut buf[..]).await,
			None => Ok(0)
		}
	}

	/// Like [`read_vectored`], except that it keeps reading until all the
	/// buffers are filled, or EOF
	///
	/// # Cancel safety
	///
	/// This function is usually cancel safe, however that depends on the exact
	/// implementation. Once the interrupt is cleared, call this function again
	/// to resume the operation.
	///
	/// [`read_vectored`]: Read::read_vectored
	async fn try_read_fully_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
		Ok(default_read_vectored(self, bufs).await?.0)
	}

	/// Same as [`try_read_fully_vectored`], except returns an [`UnexpectedEof`]
	/// error on partial reads
	///
	/// Returns the number of bytes read, which should be the same as the length
	/// of all the buffers
	///
	/// # Cancel safety
	///
	/// This function is not cancel safe. This function is not cancel safe. Data
	/// is lost on interrupt, since an error is returned.
	///
	/// [`try_read_fully_vectored`]: Read::try_read_fully_vectored
	/// [`UnexpectedEof`]: ErrorKind::UnexpectedEof
	async fn read_fully_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
		let (read, exhausted) = default_read_vectored(self, bufs).await?;

		if unlikely(!exhausted) {
			return Err(short_io_error_unless_interrupt().await);
		}

		Ok(read)
	}
}

/// Implement [`Read`] for a wrapper type.
///
/// See also [`wrapper_functions`]
///
/// [`wrapper_functions`]: crate::macros::wrapper_functions
#[macro_export]
macro_rules! read_wrapper {
	{
		inner = $inner: expr;
		mut inner = $inner_mut: expr;
	} => {
		$crate::macros::wrapper_functions! {
			inner = self.$inner;
			mut inner = self.$inner_mut;

			#[asynchronous(traitfn)]
			async fn read(&mut self, buf: &mut [u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn try_read_fully(&mut self, buf: &mut [u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_fully(&mut self, buf: &mut [u8]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_to_end(&mut self, buf: &mut Vec<u8>) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_to_string(&mut self, buf: &mut String) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			fn is_read_vectored(&self) -> bool;

			#[asynchronous(traitfn)]
			async fn read_vectored(&mut self, bufs: &mut [::std::io::IoSliceMut<'_>]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn try_read_fully_vectored(&mut self, bufs: &mut [::std::io::IoSliceMut<'_>]) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_fully_vectored(&mut self, bufs: &mut [::std::io::IoSliceMut<'_>]) -> $crate::error::Result<usize>;
		}
	}
}

pub use read_wrapper;

/// The async equivalent of [`std::io::BufRead`]
#[asynchronous(impl(mut, box))]
pub trait BufRead: Read {
	/// Fill any remaining space in the internal buffer
	/// up to `amount` total unconsumed bytes
	///
	/// Returns the number of additional bytes filled, which can be zero
	async fn fill_amount(&mut self, amount: usize) -> Result<usize>;

	/// Fill any remaining space in the internal buffer. Equivalent to calling
	/// [`fill_amount`] with the [`capacity`].
	///
	/// # Cancel safety
	///
	/// This function is usually cancel safe, however that depends on the exact
	/// implementation. Once the interrupt is cleared, call this function again
	/// to resume the operation.
	///
	/// [`fill_amount`]: BufRead::fill_amount
	/// [`capacity`]: BufRead::capacity
	async fn fill(&mut self) -> Result<usize> {
		self.fill_amount(self.capacity()).await
	}

	/// Reads all bytes into `buf` until the delimiter `byte`, an interrupt, or
	/// EOF. The ending `byte` is included in the `buf`.
	///
	/// Returns the number of bytes read, or `None` if EOF is reached
	///
	/// # Cancel safety
	///
	/// This function is usually cancel safe, however that depends on the exact
	/// implementation. Once the interrupt is cleared, call this function again
	/// to resume the operation. The number of bytes read at the interrupt can
	/// be calculated using the difference in length of `buf`
	async fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<Option<usize>> {
		let start_len = buf.len();

		loop {
			let available = self.buffer();

			/* `index` < `len`, therefore `index + 1` <= `len` */
			#[allow(clippy::arithmetic_side_effects)]
			let (used, done) = match memchr(byte, available) {
				Some(index) => (index + 1, true),
				None => (available.len(), false)
			};

			buf.extend_from_slice(&available[0..used]);

			self.consume(used);

			if done {
				break;
			}

			if self.fill().await? == 0 {
				if buf.len() == start_len {
					return Ok(None);
				}

				break;
			}
		}

		#[allow(clippy::arithmetic_side_effects)]
		Ok(Some(buf.len() - start_len))
	}

	/// Reads all bytes into `buf` until a newline (0xA byte) or EOF, and strips
	/// the line ending, or `None` if EOF is reached.
	///
	/// Returns the number of bytes read, if any
	///
	/// See also [`read_until`]
	///
	/// # Cancel safety
	///
	/// This function is not cancel safe.  On interrupt, this function cannot be
	/// called again because it may stop reading in the middle of a utf-8
	/// character, in which case it may return an invalid utf-8 error and the
	/// buf is truncated to its original length
	///
	/// [`read_until`]: BufRead::read_until
	async fn read_line(&mut self, buf: &mut String) -> Result<Option<usize>> {
		let result = append_to_string(buf, |vec: &mut Vec<u8>| async move {
			self.read_until(b'\n', vec).await
		})
		.await;

		if buf.ends_with('\n') {
			buf.pop();

			if buf.ends_with('\r') {
				buf.pop();
			}
		}

		result
	}

	/// Returns the capacity of the internal buffer
	fn capacity(&self) -> usize;

	/// Returns the remaining space in the internal buffer
	fn spare_capacity(&self) -> usize;

	/// Returns a slice to the unconsumed buffered data
	fn buffer(&self) -> &[u8];

	/// Consumes `count` bytes from the buffer
	///
	/// # Panics
	/// If `count` is greater than the number of unconsumed bytes
	///
	/// See also [`std::io::BufRead::consume`]
	fn consume(&mut self, count: usize);

	/// Unconsume `count` bytes from the buffer
	///
	/// The next call to [`Read::read`] will return the unconsumed bytes
	///
	/// # Panics
	/// If `count` is greater than the maximum number of bytes that can be
	/// unconsumed
	fn unconsume(&mut self, count: usize);

	/// Discard all data in the buffer
	fn discard(&mut self);
}

/// Implement [`BufRead`] for a wrapper type.
///
/// See also [`wrapper_functions`]
///
/// [`wrapper_functions`]: crate::macros::wrapper_functions
#[macro_export]
macro_rules! bufread_wrapper {
	{
		inner = $inner: expr;
		mut inner = $inner_mut: expr;
	} => {
		$crate::macros::wrapper_functions! {
			inner = self.$inner;
			mut inner = self.$inner_mut;

			#[asynchronous(traitfn)]
			async fn fill_amount(&mut self, amount: usize) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn fill(&mut self) -> $crate::error::Result<usize>;

			#[asynchronous(traitfn)]
			async fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> $crate::error::Result<Option<usize>>;

			#[asynchronous(traitfn)]
			async fn read_line(&mut self, buf: &mut String) -> $crate::error::Result<Option<usize>>;

			#[asynchronous(traitfn)]
			fn capacity(&self) -> usize;

			#[asynchronous(traitfn)]
			fn spare_capacity(&self) -> usize;

			#[asynchronous(traitfn)]
			fn buffer(&self) -> &[u8];

			#[asynchronous(traitfn)]
			fn consume(&mut self, count: usize);

			#[asynchronous(traitfn)]
			fn unconsume(&mut self, count: usize);

			#[asynchronous(traitfn)]
			fn discard(&mut self);
		}
	}
}

pub use bufread_wrapper;

/// Utility trait for converting a [`BufRead`] instance to a [`Lines`]
pub trait IntoLines: BufReadSealed {
	/// Returns an iterator over the lines of this reader
	///
	/// See also [`std::io::BufRead::lines`]
	fn lines(self) -> Lines<Self>
	where
		Self: Sized
	{
		Lines::new(self)
	}
}

impl<T: BufReadSealed> IntoLines for T {}

/// An iterator over the lines of an instance of [`BufRead`]
pub struct Lines<R>(R);

impl<R: BufRead> Lines<R> {
	pub const fn new(reader: R) -> Self {
		Self(reader)
	}
}

#[asynchronous]
impl<R: BufRead> AsyncIterator for Lines<R> {
	type Item = Result<String>;

	async fn next(&mut self) -> Option<Self::Item> {
		let mut line = String::new();
		let read = self.0.read_line(&mut line).await.transpose()?;

		Some(read.map(|_| line))
	}
}
