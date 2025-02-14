//! Contains the [`BufReader`] implementation

use std::ops::Range;

use super::*;
use crate::impls::UintExt;

macro_rules! identity {
	($buffered:expr) => {
		$buffered
	};
}

macro_rules! deref {
	($buffered:expr) => {
		*$buffered
	};
}

macro_rules! impl_bufread {
	($(<$lt:lifetime>)? $type:ident, $buffered:ident) => {
		#[asynchronous]
		#[allow(single_use_lifetimes)]
		impl< $($lt,)? R: Read + ?Sized> $type< $($lt,)? R> {
			/// Reads from our internal buffer into `buf`
			fn read_into(&mut self, buf: &mut [u8]) -> usize {
				let len = read_into_slice(buf, self.buffer());

				#[cfg(feature = "tracing")]
				crate::trace!(
					target: &*self,
					"## read(buf = &mut [u8; {}]) = Buffered({} / {})",
					buf.len(),
					len,
					self.buffer().len()
				);

				#[allow(clippy::arithmetic_side_effects)]
				(self.buffered.start += len);

				len
			}

			async fn fill_buf_range(&mut self, range: Range<usize>) -> Result<usize> {
				let buf = &mut self.data[range.clone()];
				let read = self.reader.read(buf).await?;

				if read != 0 {
					#[allow(clippy::arithmetic_side_effects)]
					(self.buffered.end = range.start + length_check(buf, read));
				}

				#[cfg(feature = "tracing")]
				crate::trace!(target: &*self, "## fill_buf_range(range = {:?}) = Ok({})", range, read);

				Ok(read)
			}

			async fn fill_buf(&mut self) -> Result<usize> {
				let read = self.fill_buf_range(0..self.data.len()).await?;

				if read != 0 {
					self.buffered.start = 0;
				}

				Ok(read)
			}

			#[cold]
			async fn read_cold(&mut self, buf: &mut [u8]) -> Result<usize> {
				if buf.len() >= self.capacity() {
					let read = self.reader.read(buf).await?;

					#[cfg(feature = "tracing")]
					crate::trace!(target: &*self, "## read(buf = &mut [u8; {}]) = Direct({})", buf.len(), read);

					return Ok(read);
				}

				Ok(if self.fill_buf().await? != 0 {
					self.read_into(buf)
				} else {
					0
				})
			}

			/// Shift unconsumed bytes to the beginning to make space for calls to
			/// [`fill`] without discarding any unconsumed data
			///
			/// [`fill`]: BufRead::fill
			pub fn move_data_to_beginning(&mut self) {
				if self.buffered.start == 0 {
					return;
				}

				let len = self.buffer().len();

				if len == 0 {
					self.discard();
				} else {
					self.data.copy_within(self.buffered.clone(), 0);
					$buffered!(self.buffered) = 0..len;
				}
			}

			/// The current position into the internal buffer
			pub const fn position(&self) -> usize {
				self.buffered.start
			}
		}

		#[asynchronous]
		#[allow(single_use_lifetimes)]
		impl< $($lt,)? R: Read + ?Sized> Read for $type< $($lt,)? R> {
			async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
				if !self.buffer().is_empty() {
					return Ok(self.read_into(buf));
				}

				self.read_cold(buf).await
			}
		}

		#[asynchronous]
		#[allow(single_use_lifetimes)]
		impl< $($lt,)? R: Read + ?Sized> BufRead for $type< $($lt,)? R> {
			async fn fill_amount(&mut self, amount: usize) -> Result<usize> {
				assert!(amount <= self.capacity());

				let mut start = self.buffered.end;

				/* cannot overflow here due to limits of buf's length */
				#[allow(clippy::arithmetic_side_effects)]
				let mut end = self.buffered.start + amount;

				if end <= start {
					return Ok(0);
				}

				if end > self.capacity() {
					end = amount;

					if $buffered!(self.buffered).is_empty() {
						/* try not to discard existing data if read returns EOF, assuming the read
						 * impl doesn't write junk even when returning zero
						 */
						start = 0;
					} else {
						self.move_data_to_beginning();

						if self.spare_capacity() == 0 {
							return Ok(0);
						}

						start = self.buffered.end;
					}
				}

				let read = self.fill_buf_range(start..end).await?;

				if start == 0 && read != 0 {
					/* read new data at beginning, reset pos */
					self.buffered.start = 0;
				}

				Ok(read)
			}

			fn capacity(&self) -> usize {
				self.data.len()
			}

			fn spare_capacity(&self) -> usize {
				#[allow(clippy::arithmetic_side_effects)]
				(self.data.len() - self.buffered.end)
			}

			#[allow(unsafe_code)]
			fn buffer(&self) -> &[u8] {
				/* Safety: `self.buffered` is always valid and in range */
				unsafe { self.data.get_unchecked(self.buffered.clone()) }
			}

			#[allow(clippy::arithmetic_side_effects)]
			fn consume(&mut self, count: usize) {
				assert!(count <= self.buffer().len());

				self.buffered.start += count;
			}

			fn unconsume(&mut self, count: usize) {
				self.buffered.start = self
					.buffered
					.start
					.checked_sub(count)
					.expect("`count` > `self.position()`");
			}

			fn discard(&mut self) {
				$buffered!(self.buffered) = 0..0;
			}
		}

		#[asynchronous]
		#[allow(single_use_lifetimes)]
		impl< $($lt,)? R: Read + Seek + ?Sized> $type< $($lt,)? R> {
			async fn seek_relative(&mut self, rel: i64) -> Result<u64> {
				#[allow(clippy::never_loop)]
				loop {
					let Some(pos) = rel.checked_add_unsigned(self.buffered.start as u64) else {
						break;
					};

					/* wrap cannot happen due to limits of buf's len */
					#[allow(clippy::cast_possible_wrap)]
					if pos < 0 || pos > self.buffered.end as i64 {
						break;
					}

					#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
					(self.buffered.start = pos as usize);

					return self.stream_position().await;
				}

				self.seek_inner(SeekFrom::Current(rel)).await
			}

			async fn seek_inner(&mut self, mut seek: SeekFrom) -> Result<u64> {
				if let SeekFrom::Current(pos) = &mut seek {
					/* wrap cannot happen due to limits of buf's len */
					#[allow(clippy::cast_possible_wrap)]
					let remainder = self.buffer().len() as i64;

					if let Some(p) = pos.checked_sub(remainder) {
						*pos = p;
					} else {
						#[allow(clippy::arithmetic_side_effects)]
						self.reader.seek(SeekFrom::Current(-remainder)).await?;
					}
				}

				let pos = self.reader.seek(seek).await;

				self.discard();

				#[cfg(feature = "tracing")]
				crate::trace!(target: &*self, "## seek_inner(seek = {:?}) = {:?}", seek, pos);

				pos
			}

			async fn seek_abs(&mut self, abs: u64, seek: SeekFrom) -> Result<u64> {
				let stream_pos = self.stream_position().await?;

				#[allow(unstable_name_collisions)]
				if let Some(rel) = abs.checked_signed_diff(stream_pos) {
					self.seek_relative(rel).await
				} else {
					self.seek_inner(seek).await
				}
			}
		}

		#[asynchronous]
		#[allow(single_use_lifetimes)]
		impl< $($lt,)? R: Read + Seek + ?Sized> Seek for $type< $($lt,)? R> {
			fn stream_len_fast(&self) -> bool {
				self.reader.stream_len_fast()
			}

			async fn stream_len(&mut self) -> Result<u64> {
				self.reader.stream_len().await
			}

			fn stream_position_fast(&self) -> bool {
				self.reader.stream_position_fast()
			}

			/// # Panics
			/// If there was an overflow calculating the stream position
			async fn stream_position(&mut self) -> Result<u64> {
				let pos = self.reader.stream_position().await?;
				let remaining = self.buffer().len();

				Ok(pos
					.checked_sub(remaining as u64)
					.expect("Overflow occurred calculating stream position"))
			}

			/// # Panics
			/// If there was an overflow calculating the new position
			async fn seek(&mut self, seek: SeekFrom) -> Result<u64> {
				match seek {
					SeekFrom::Current(pos) => self.seek_relative(pos).await,
					_ if !self.stream_position_fast() => self.seek_inner(seek).await,
					SeekFrom::Start(pos) => self.seek_abs(pos, seek).await,
					_ if !self.stream_len_fast() => self.seek_inner(seek).await,
					SeekFrom::End(pos) => {
						let new_pos = self
							.stream_len()
							.await?
							.checked_add_signed(pos)
							.expect("Overflow occured calculating absolute offset");

						self.seek_abs(new_pos, seek).await
					}
				}
			}
		}
	}
}

/// The async equivalent of [`std::io::BufReader`]
pub struct BufReader<R: ?Sized> {
	data: Box<[u8]>,
	buffered: Range<usize>,
	reader: R
}

#[asynchronous]
impl<R: ?Sized> BufReader<R> {
	/// Creates a new `BufReader<R>` with a [`DEFAULT_BUFFER_SIZE`]
	pub fn new(reader: R) -> Self
	where
		R: Sized
	{
		Self::with_capacity(reader, DEFAULT_BUFFER_SIZE)
	}

	/// Creates a new `BufReader<R>` with the specified buffer capacity
	pub fn with_capacity(reader: R, capacity: usize) -> Self
	where
		R: Sized
	{
		Self::from_parts(reader, Vec::with_capacity(capacity), 0)
	}

	/// Creates a new `BufReader<R>` from parts
	///
	/// # Panics
	/// If `pos > buf.len()`
	pub fn from_parts(reader: R, mut buf: Vec<u8>, pos: usize) -> Self
	where
		R: Sized
	{
		let len = buf.len();

		assert!(pos <= len);

		buf.resize(buf.capacity(), 0);

		Self {
			reader,
			data: buf.into_boxed_slice(),
			buffered: pos..len
		}
	}

	/// Unwraps this `BufReader<R>`, returning the underlying reader
	///
	/// Any leftover data in the internal buffer is lost. A subsequent
	/// read from the underlying reader may lead to data loss
	pub fn into_inner(self) -> R
	where
		R: Sized
	{
		self.reader
	}

	/// Get a reference to the underlying reader
	pub const fn inner(&self) -> &R {
		&self.reader
	}

	/// Get a reference to the underlying reader
	pub fn inner_mut(&mut self) -> &mut R {
		&mut self.reader
	}

	/// Unwraps this `BufReader<R>`, returning its parts
	///
	/// The `Vec<u8>` contains the buffered data, and the `usize` is the
	/// position to start reading from
	pub fn into_parts(self) -> (R, Vec<u8>, usize)
	where
		R: Sized
	{
		let mut buf = self.data.into_vec();

		buf.truncate(self.buffered.end);

		(self.reader, buf, self.buffered.start)
	}
}

impl_bufread!(BufReader, identity);

pub struct BufReadHalf<'a, R: ?Sized> {
	data: &'a mut [u8],
	buffered: &'a mut Range<usize>,
	reader: R
}

impl<R: ?Sized> BufReadHalf<'_, R> {
	pub const fn inner(&self) -> &R {
		&self.reader
	}

	pub fn inner_mut(&mut self) -> &mut R {
		&mut self.reader
	}
}

impl_bufread!(<'a> BufReadHalf, deref);

impl<R: Read + SplitMut + ?Sized> SplitMut for BufReader<R> {
	type Reader<'a> = BufReadHalf<'a, R::Reader<'a>> where Self: 'a;
	type Writer<'a> = R::Writer<'a> where Self: 'a;

	fn try_split(&mut self) -> Result<(Self::Reader<'_>, Self::Writer<'_>)> {
		let (reader, writer) = self.reader.try_split()?;

		Ok((
			BufReadHalf {
				data: &mut self.data,
				buffered: &mut self.buffered,
				reader
			},
			writer
		))
	}
}
