use std::fmt::Debug;

use crate::macros::{panic_nounwind, sealed_trait, unreachable_unchecked};

sealed_trait!();

pub trait ResultExt<T, E>: Sealed + Sized {
	fn expect_nounwind(self, msg: &str) -> T
	where
		E: Debug;

	/// # Safety
	/// `self` must be `Some`
	unsafe fn expect_unchecked(self, msg: &str) -> T
	where
		E: Debug;
}

impl<T, E> Sealed for Result<T, E> {}

impl<T, E> ResultExt<T, E> for Result<T, E> {
	#[track_caller]
	fn expect_nounwind(self, msg: &str) -> T
	where
		E: Debug
	{
		match self {
			Ok(val) => val,
			Err(err) => panic_nounwind!("{}: {:?}", msg, err)
		}
	}

	#[track_caller]
	unsafe fn expect_unchecked(self, msg: &str) -> T
	where
		E: Debug
	{
		match self {
			Ok(val) => val,

			/* Safety: guaranteed by caller */
			Err(err) => unsafe { unreachable_unchecked!("{}: {:?}", msg, err) }
		}
	}
}
