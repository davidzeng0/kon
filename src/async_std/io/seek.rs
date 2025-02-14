//! Contains traits and utilities for seeking a stream

use super::*;

/// The async equivalent of [`std::io::Seek`]
#[asynchronous(impl(mut, box))]
pub trait Seek {
	/// Seek to an offset, in bytes
	///
	/// If the seek operation completed successfully, this method returns the
	/// new position from the start of the stream
	///
	/// See also [`std::io::Seek::seek`]
	///
	/// # Cancel safety
	///
	/// The cancel safety of this function depends on the implementation.
	async fn seek(&mut self, seek: SeekFrom) -> Result<u64>;

	/// Returns true if the `Seek` implementation has an efficient
	/// [`stream_len`] implementation
	///
	/// [`stream_len`]: Seek::stream_len
	fn stream_len_fast(&self) -> bool {
		false
	}

	/// Get the length of the stream in bytes
	///
	/// See also [`std::io::Seek::stream_len`]
	///
	/// # Cancel safety
	///
	/// The cancel safety of this function depends on the implementation.
	async fn stream_len(&mut self) -> Result<u64> {
		let old_pos = self.stream_position().await?;
		let len = self.seek(SeekFrom::End(0)).await?;

		if old_pos != len {
			self.seek(SeekFrom::Start(old_pos)).await?;
		}

		Ok(len)
	}

	/// Returns true if the `Seek` implementation has an efficient
	/// [`stream_position`] implementation
	///
	/// [`stream_position`]: Seek::stream_position
	fn stream_position_fast(&self) -> bool {
		false
	}

	/// Get the current position in the stream in bytes
	///
	/// See also [`std::io::Seek::stream_position`]
	///
	/// # Cancel safety
	///
	/// The cancel safety of this function depends on the implementation.
	async fn stream_position(&mut self) -> Result<u64> {
		self.seek(SeekFrom::Current(0)).await
	}

	/// Rewinds the stream to the beginning
	///
	/// This is a convenience method, equivalent to `seek(SeekFrom::Start(0))`
	///
	/// # Cancel safety
	///
	/// This function is usually cancel safe, however that depends on the exact
	/// implementation. Once the interrupt is cleared, call this function again
	/// to resume the operation.
	async fn rewind(&mut self) -> Result<()> {
		self.seek(SeekFrom::Start(0)).await?;

		Ok(())
	}

	/// Rewind `amount` bytes on the stream
	///
	/// On interrupt, the amount rewinded can be calculated as the difference in
	/// the stream position.
	///
	/// # Cancel safety
	///
	/// This function is usually cancel safe, however that depends on the exact
	/// implementation. Once the interrupt is cleared, call this function again
	/// to resume the operation.
	async fn rewind_amount(&mut self, amount: u64) -> Result<u64> {
		if let Ok(amount) = i64::try_from(amount) {
			/* amount is positive */
			#[allow(clippy::arithmetic_side_effects)]
			self.seek(SeekFrom::Current(-amount)).await
		} else {
			self.seek(SeekFrom::Current(i64::MIN)).await?;

			#[allow(clippy::arithmetic_side_effects)]
			let remaining = -(i64::MIN).wrapping_add_unsigned(amount);

			self.seek(SeekFrom::Current(remaining)).await
		}
	}

	/// Skips `amount` bytes from the stream
	///
	/// On interrupt, the amount skipped can be calculated as the difference in
	/// the stream position.
	///
	/// # Cancel safety
	///
	/// This function is usually cancel safe, however that depends on the exact
	/// implementation. Once the interrupt is cleared, call this function again
	/// to resume the operation.
	async fn skip_amount(&mut self, amount: u64) -> Result<u64> {
		if let Ok(amount) = i64::try_from(amount) {
			return self.seek(SeekFrom::Current(amount)).await;
		}

		let mut left = amount;

		/* this can loop up to 3 times :( */
		loop {
			#[allow(clippy::cast_possible_wrap)]
			let amount = left.max(i64::MAX as u64) as i64;
			let result = self.seek(SeekFrom::Current(amount)).await?;

			#[allow(clippy::arithmetic_side_effects, clippy::cast_sign_loss)]
			(left -= amount as u64);

			if left == 0 {
				break Ok(result);
			}
		}
	}
}

/// Implement [`Seek`] for a wrapper type.
///
/// See also [`wrapper_functions`]
///
/// [`wrapper_functions`]: crate::macros::wrapper_functions
#[macro_export]
macro_rules! seek_wrapper {
	{
		inner = $inner: expr;
		mut inner = $inner_mut: expr;
	} => {
		$crate::macros::wrapper_functions! {
			inner = self.$inner;
			mut inner = self.$inner_mut;

			#[asynchronous(traitfn)]
			async fn seek(&mut self, seek: ::std::io::SeekFrom) -> $crate::error::Result<u64>;

			#[asynchronous(traitfn)]
			fn stream_len_fast(&self) -> bool;

			#[asynchronous(traitfn)]
			async fn stream_len(&mut self) -> $crate::error::Result<u64>;

			#[asynchronous(traitfn)]
			fn stream_position_fast(&self) -> bool;

			#[asynchronous(traitfn)]
			async fn stream_position(&mut self) -> $crate::error::Result<u64>;

			#[asynchronous(traitfn)]
			async fn rewind(&mut self) -> $crate::error::Result<()>;

			#[asynchronous(traitfn)]
			async fn rewind_amount(&mut self, amount: u64) -> $crate::error::Result<u64>;

			#[asynchronous(traitfn)]
			async fn skip_amount(&mut self, amount: u64) -> $crate::error::Result<u64>;
		}
	}
}

pub use seek_wrapper;
