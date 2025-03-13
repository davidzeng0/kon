use std::env::var;
use std::fmt;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};

use ctor::ctor;
use lazy_static::lazy_static;
use log::{set_boxed_logger, Log, Metadata, Record};

use super::*;
use crate::impls::ResultExt;
use crate::{error, trace};

lazy_static! {
	static ref STDERR: Mutex<BufWriter<Stderr>> =
		Mutex::new(BufWriter::with_capacity(1024, stderr()));
}

#[allow(clippy::missing_panics_doc)]
fn get_stderr() -> MutexGuard<'static, BufWriter<Stderr>> {
	#[allow(clippy::unwrap_used)]
	STDERR.lock().unwrap()
}

macro_rules! ansi_color {
	(bold) => {
		format_args!("{}", "\x1b[1m")
	};

	($color:expr) => {
		format_args!("\x1b[1;48;5;{}m", $color)
	};

	() => {
		format_args!("{}", "\x1b[0m")
	};
}

struct Adapter<'a> {
	output: MutexGuard<'a, BufWriter<Stderr>>,
	record: &'a Record<'a>,
	wrote_prefix: bool
}

impl Adapter<'_> {
	fn write_prefix_with_color(&mut self, color: Arguments<'_>) -> Result<()> {
		self.output.write_fmt(format_args!(
			"{}| {: >24} |{} ",
			color,
			self.record.target(),
			ansi_color!()
		))
	}

	fn write_prefix(&mut self) -> Result<()> {
		if self.wrote_prefix {
			return Ok(());
		}

		let result = match self.record.level() {
			Level::Error => self.write_prefix_with_color(ansi_color!(1)),
			Level::Warn => self.write_prefix_with_color(ansi_color!(11)),
			Level::Info => self.write_prefix_with_color(ansi_color!(10)),
			Level::Debug => self.write_prefix_with_color(ansi_color!(14)),
			Level::Trace => self.write_prefix_with_color(ansi_color!(bold))
		};

		self.wrote_prefix = true;

		result
	}
}

impl fmt::Write for Adapter<'_> {
	fn write_str(&mut self, data: &str) -> fmt::Result {
		if !data.contains('\n') {
			self.write_prefix().map_err(|_| fmt::Error)?;
			self.output
				.write_all(data.as_bytes())
				.map_err(|_| fmt::Error)?;

			return Ok(());
		}

		for line in data.split_inclusive('\n') {
			self.write_prefix().map_err(|_| fmt::Error)?;
			self.output
				.write_all(line.as_bytes())
				.map_err(|_| fmt::Error)?;
			if line.ends_with('\n') {
				self.wrote_prefix = false;
			}
		}

		Ok(())
	}
}

struct Logger;

impl Log for Logger {
	fn enabled(&self, _: &Metadata<'_>) -> bool {
		true
	}

	fn log(&self, record: &Record<'_>) {
		if !self.enabled(record.metadata()) {
			return;
		}

		let mut adapter = Adapter { output: get_stderr(), record, wrote_prefix: false };

		let _ = fmt::Write::write_fmt(&mut adapter, *record.args());

		if adapter.wrote_prefix {
			let _ = adapter.output.write_all(b"\n");
		}

		let _ = adapter.output.flush();
	}

	fn flush(&self) {
		let _ = get_stderr().flush();
	}
}

#[track_caller]
fn panic_hook(info: &PanicHookInfo<'_>) {
	let msg = match info.payload().downcast_ref::<&'static str>() {
		Some(s) => *s,
		None => match info.payload().downcast_ref::<String>() {
			Some(s) => &s[..],
			None => "Box<dyn Any>"
		}
	};

	print_panic(info.location(), format_args!("{}", msg));
}

#[ctor]
fn init() {
	set_boxed_logger(Box::new(Logger)).expect_nounwind("Failed to initialize logger");

	#[cfg(feature = "panic-log")]
	set_hook(Box::new(panic_hook));

	let level = match var("XX_LOG") {
		Ok(level) => LevelFilter::from_str(&level).map_err(|_| Some(level)),
		Err(_) => Err(None)
	};

	match level {
		Ok(level) => {
			set_max_level(level);

			trace!(
				"== Log level to {} by environment variables",
				level.as_str()
			);
		}

		Err(input) => {
			set_max_level(LevelFilter::Info);

			if let Some(input) = input {
				error!("== Invalid log level {}", input);
			}
		}
	}
}
