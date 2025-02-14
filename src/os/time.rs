use super::error::result_from_libc;
use super::*;
use crate::impls::OptionExt;

define_struct! {
	pub struct TimeVal {
		pub sec: i64,
		pub micros: i64
	}
}

define_struct! {
	pub struct TimeSpec {
		pub sec: i64,
		pub nanos: i64
	}
}

#[allow(clippy::unwrap_used)]
impl TimeSpec {
	#[must_use]
	pub const fn indefinite() -> Self {
		Self { sec: -1, nanos: -1 }
	}

	#[must_use]
	pub const fn zero() -> Self {
		Self { sec: 0, nanos: 0 }
	}

	#[must_use]
	#[allow(clippy::cast_possible_wrap, clippy::arithmetic_side_effects)]
	pub const fn from_ms(ms: u64) -> Self {
		let subsec_millis: i64 = (ms % 1000) as i64;

		Self {
			sec: (ms / 1000) as i64,
			nanos: subsec_millis * 1_000_000
		}
	}

	/// # Panics
	/// if the input cannot fit in the resulting Duration
	#[must_use]
	#[allow(clippy::cast_possible_wrap)]
	pub const fn from_nanos(duration: u64) -> Self {
		Self {
			sec: (duration / 1_000_000_000) as i64,
			nanos: (duration % 1_000_000_000) as i64
		}
	}

	/// # Panics
	/// if the input cannot fit in the resulting Duration
	#[must_use]
	pub fn from_duration(duration: Duration) -> Self {
		Self {
			sec: duration.as_secs().try_into().unwrap(),
			nanos: duration.subsec_nanos() as i64
		}
	}

	#[must_use]
	#[allow(clippy::arithmetic_side_effects)]
	pub fn try_as_nanos(&self) -> Option<u128> {
		let sec: u128 = self.sec.try_into().ok()?;
		let nanos: u128 = self.nanos.try_into().ok()?;

		Some(nanos + (sec * 1_000_000_000))
	}

	/// # Panics
	/// if this timespec is not a valid duration
	#[must_use]
	#[allow(clippy::expect_used)]
	pub fn as_nanos(&self) -> u128 {
		self.try_as_nanos().expect("Invalid duration")
	}
}

define_enum! {
	#[repr(i32)]
	pub enum ClockId {
		RealTime,
		Monotonic,
		ProcessCpuTimeId,
		ThreadCpuTimeId,
		MonotonicRaw,
		RealTimeCoarse,
		MonotonicCoarse,
		BootTime,
		RealTimeAlarm,
		BootTimeAlarm,
		Tai = 11
	}
}

extern "C" {
	fn clock_gettime(clock: ClockId, spec: &mut TimeSpec) -> i32;
}

pub fn time(clock: ClockId) -> Result<TimeSpec> {
	let mut ts = TimeSpec { sec: 0, nanos: 0 };

	/* Safety: &mut ts is a valid pointer */
	result_from_libc(unsafe { clock_gettime(clock, &mut ts) } as isize)?;

	Ok(ts)
}

pub fn nanotime(clock: ClockId) -> Result<u64> {
	let ts = time(clock)?;
	let nanos = ts
		.try_as_nanos()
		.and_then(|nanos| -> Option<u64> { nanos.try_into().ok() });

	/* time on linux fits into a u64 */
	Ok(nanos.expect_nounwind("Failed to get the time"))
}
