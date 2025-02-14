use std::cmp;
use std::fmt::{self, Debug, Formatter, Result};
use std::num::NonZeroUsize;
use std::ops::{Deref, DerefMut};
use std::ptr::{self as pointer, null_mut, slice_from_raw_parts_mut};
use std::rc::Rc;
use std::sync::Arc;

use crate::macros::{assert_unsafe_precondition, sealed_trait, wrapper_functions};

#[doc(hidden)]
pub mod internal;
pub mod non_null;
pub mod pin;
pub mod ptr;

pub use std::mem::offset_of;

#[doc(inline)]
pub use non_null::*;
#[doc(inline)]
pub use pin::*;
#[doc(inline)]
pub use ptr::*;

#[macro_export]
macro_rules! container_of {
	($ptr:expr, $type:ty => $field:ident) => {
		$crate::pointer::Pointer::cast::<u8>($ptr)
			.sub(::std::mem::offset_of!($type, $field))
			.cast::<$type>()
	};
}

pub use container_of;
