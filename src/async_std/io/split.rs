//! Contains traits for splitting a stream into read and write halves

use super::*;

/// Splits a stream into a borrowed read half and a write half.
pub trait SplitMut {
	type Reader<'a>: Read
	where
		Self: 'a;

	type Writer<'a>: Write
	where
		Self: 'a;

	fn split(&mut self) -> (Self::Reader<'_>, Self::Writer<'_>) {
		self.try_split().unwrap()
	}

	fn try_split(&mut self) -> Result<(Self::Reader<'_>, Self::Writer<'_>)>;
}

/// Splits a stream into a read half and a write half.
pub trait Split {
	type Reader: Read;
	type Writer: Write;

	fn try_split(self) -> Result<(Self::Reader, Self::Writer)>;
}
