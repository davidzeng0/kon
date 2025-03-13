use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Default)]
/* xx-core only supports x86_64 and aarch64 */
#[repr(align(128))]
pub struct CachePadded<T>(pub T);

impl<T> Deref for CachePadded<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.0
	}
}

impl<T> DerefMut for CachePadded<T> {
	fn deref_mut(&mut self) -> &mut T {
		&mut self.0
	}
}
