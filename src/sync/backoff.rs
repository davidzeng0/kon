use std::hint::spin_loop;
use std::thread::yield_now;

const SPIN_LIMIT: u16 = 6;
const YIELD_LIMIT: u16 = 10;

#[derive(Clone)]
#[allow(missing_copy_implementations)]
pub struct Backoff {
	step: u16
}

impl Backoff {
	#[must_use]
	pub const fn new() -> Self {
		Self { step: 0 }
	}

	pub fn reset(&mut self) {
		self.step = 0;
	}

	fn spin_internal(&self) {
		for _ in 0..1 << self.step.min(SPIN_LIMIT) {
			spin_loop();
		}
	}

	#[allow(clippy::arithmetic_side_effects)]
	pub fn spin(&mut self) {
		self.spin_internal();

		if self.step <= SPIN_LIMIT {
			self.step += 1;
		}
	}

	#[allow(clippy::arithmetic_side_effects)]
	pub fn snooze(&mut self) {
		if self.step < SPIN_LIMIT {
			self.spin_internal();
		} else {
			yield_now();
		}

		if self.step <= YIELD_LIMIT {
			self.step += 1;
		}
	}

	#[must_use]
	pub const fn is_completed(&self) -> bool {
		self.step > YIELD_LIMIT
	}

	#[must_use]
	pub const fn step(&self) -> u16 {
		self.step
	}
}

impl Default for Backoff {
	fn default() -> Self {
		Self::new()
	}
}
