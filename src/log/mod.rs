use std::backtrace::{Backtrace, BacktraceStatus};
use std::fmt::Arguments;
use std::io::{stderr, BufWriter, Result, Stderr, Write};
use std::panic::{set_hook, Location, PanicHookInfo};

pub use log::{max_level as get_max_level, set_max_level, Level, LevelFilter};

#[doc(hidden)]
pub mod internal;
#[cfg(feature = "logger")]
mod logger;
mod macros;

macro_rules! get_thread_name {
	($var:ident) => {
		let thread = ::std::thread::current();
		let $var = thread.name().unwrap_or("<unnamed>");
	};
}

pub fn print_backtrace() {
	get_thread_name!(thread_name);

	internal::print_backtrace(thread_name);
}

pub fn print_fatal(fmt: Arguments<'_>) {
	get_thread_name!(thread_name);

	internal::print_fatal(thread_name, fmt);
}

#[track_caller]
pub fn print_panic(location: Option<&Location<'_>>, fmt: Arguments<'_>) {
	get_thread_name!(thread_name);

	let location = if let Some(location) = location {
		location
	} else {
		Location::caller()
	};

	internal::print_fatal(
		thread_name,
		format_args!("Panic occurred at {}:\n>> {}", location, fmt)
	);

	let backtrace = Backtrace::capture();

	if backtrace.status() == BacktraceStatus::Captured {
		internal::print_fatal(thread_name, format_args!("\nBack trace:\n{}", backtrace));
	} else {
		internal::print_fatal(
			thread_name,
			format_args!(
				"note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"
			)
		);
	}
}
