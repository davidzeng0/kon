use crate::macros::{macro_each, paste};

pub trait IntoBytes<const N: usize> {
	fn into_bytes(self) -> [u8; N];
}

pub trait FromBytes<const N: usize> {
	fn from_bytes(bytes: [u8; N]) -> Self;
}

impl<const N: usize> IntoBytes<N> for [u8; N] {
	fn into_bytes(self) -> [u8; N] {
		self
	}
}

impl<const N: usize> FromBytes<N> for [u8; N] {
	fn from_bytes(bytes: [u8; N]) -> Self {
		bytes
	}
}

macro_rules! impl_primitive_bytes_encoding_endian {
	($type:ty, $endian:ident, $trait_endian:ident) => {
		paste! {
			#[allow(non_camel_case_types)]
			#[derive(Clone, Copy)]
			pub struct [<$type $endian>](pub $type);

			impl IntoBytes<{ size_of::<$type>() }> for [<$type $endian>] {
				fn into_bytes(self) -> [u8; size_of::<$type>()] {
					self.0.[<to_ $endian _bytes>]()
				}
			}

			impl FromBytes<{ size_of::<$type>() }> for [<$type $endian>] {
				fn from_bytes(bytes: [u8; size_of::<$type>()]) -> Self {
					Self($type::[<from_ $endian _bytes>](bytes))
				}
			}

			#[allow(dead_code)]
			impl [<$type $endian>] {
				pub const BYTES: usize = size_of::<$type>();
			}
		}
	};
}

macro_rules! impl_primitive_type {
	($type:ty, $bits:literal) => {
		impl_primitive_bytes_encoding_endian!($type, le, LittleEndian);
		impl_primitive_bytes_encoding_endian!($type, be, BigEndian);
	};
}

macro_rules! impl_int {
	($bits:literal) => {
		paste! {
			impl_primitive_type!([<i $bits>], $bits);
			impl_primitive_type!([<u $bits>], $bits);
		}
	};
}

/* usize and isize omitted intentionally */
macro_each!(impl_int, 8, 16, 32, 64, 128);
impl_primitive_type!(f32, 32);
impl_primitive_type!(f64, 64);
