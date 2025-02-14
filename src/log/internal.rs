use std::any::type_name;
use std::str::from_utf8;

pub use log::{log, log_enabled};

use super::*;
use crate::io::UninitBuf;
use crate::pointer::*;

#[allow(
	clippy::unwrap_used,
	clippy::arithmetic_side_effects,
	clippy::missing_panics_doc
)]
fn get_struct_name<T>() -> &'static str
where
	T: ?Sized
{
	let full_name = type_name::<T>();

	let mut start = None;
	let mut angle_brackets = 0;
	let mut segment = full_name;

	for (index, ch) in full_name.char_indices() {
		start.get_or_insert(index);

		if ch == '<' {
			if angle_brackets == 0 {
				segment = &full_name[start.unwrap()..index];
			}

			angle_brackets += 1;
		}

		if ch == '>' {
			angle_brackets -= 1;

			if angle_brackets == 0 {
				start = None;
			}
		}
	}

	segment.split("::").last().unwrap()
}

fn get_struct_addr_low<T, const MUT: bool>(val: Pointer<T, MUT>) -> usize
where
	T: ?Sized
{
	val.addr() & u32::MAX as usize
}

#[allow(clippy::impl_trait_in_params)]
pub fn format_struct<T, const MUT: bool>(
	write: &mut impl Write, addr: Pointer<T, MUT>, name: &str
) -> Result<()>
where
	T: ?Sized
{
	write.write_fmt(format_args!(
		"@ {:0>8x} {: >13}",
		get_struct_addr_low(addr),
		name
	))
}

pub fn log_struct<T, const MUT: bool>(
	level: Level, addr: Pointer<T, MUT>, name: &str, args: Arguments<'_>
) where
	T: ?Sized
{
	let mut fmt_buf = UninitBuf::<64>::new();
	let _ = format_struct(&mut fmt_buf, addr, name);

	log!(
		target: from_utf8(&fmt_buf).unwrap_or("<error>"),
		level,
		"{}",
		args
	);
}

#[inline(never)]
#[cold]
pub fn log_target<T, const MUT: bool>(level: Level, target: Pointer<T, MUT>, args: Arguments<'_>)
where
	T: ?Sized
{
	log_struct(level, target, get_struct_name::<T>(), args);
}

pub(super) fn print_fatal(thread_name: &str, fmt: Arguments<'_>) {
	log!(target: thread_name, Level::Error, "{}", fmt);
}

pub(super) fn print_backtrace(thread_name: &str) {
	let backtrace = Backtrace::capture();

	log!(target: thread_name, Level::Error, "{:?}", backtrace);
}
