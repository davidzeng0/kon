use std::fmt::Arguments;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

use crate::log::*;

pub type MaybePanic<T> = std::thread::Result<T>;

pub fn catch_unwind_safe<F, Output>(func: F) -> MaybePanic<Output>
where
	F: FnOnce() -> Output
{
	catch_unwind(AssertUnwindSafe(func))
}

#[track_caller]
#[cold]
#[inline(never)]
pub fn panic_nounwind(fmt: Arguments<'_>) -> ! {
	print_panic(None, fmt);
	print_fatal(format_args!("Non unwinding panic, aborting"));

	std::process::abort();
}

/// # Panics
/// resumes the panic if `result` is an `Err`
pub fn join<T>(result: MaybePanic<T>) -> T {
	match result {
		Ok(ok) => ok,
		Err(err) => resume_unwind(err)
	}
}

#[inline(always)]
pub fn call_no_unwind<F, Output>(func: F) -> Output
where
	F: FnOnce() -> Output
{
	#[cfg(debug_assertions)]
	match catch_unwind_safe(func) {
		Ok(ok) => ok,
		Err(_) => crate::macros::panic_nounwind!("Function that must never panic panicked")
	}

	#[cfg(not(debug_assertions))]
	func()
}
