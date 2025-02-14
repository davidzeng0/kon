use std::sync::Mutex;

use crate::fiber::*;
use crate::impls::OptionExt;
use crate::trace;

struct Data {
	pool: Vec<Fiber>,
	active: u64
}

impl Data {
	const fn new() -> Self {
		Self { pool: Vec::new(), active: 0 }
	}
}

pub struct Pool {
	data: Mutex<Data>
}

impl Pool {
	#[must_use]
	pub const fn new() -> Self {
		Self { data: Mutex::new(Data::new()) }
	}

	/// # Panics
	/// if creating a fiber fails
	#[must_use]
	pub fn new_fiber(&self, start: Start) -> Fiber {
		let fiber = {
			/* we never panic with the lock */
			#[allow(clippy::unwrap_used)]
			let mut data = self.data.lock().unwrap();

			data.active = data
				.active
				.checked_add(1)
				.expect_nounwind("Fatal error: fiber count overflow");
			data.pool.pop()
		};

		match fiber {
			Some(mut fiber) => {
				trace!(target: self, "== Reusing stack for worker");

				/* Safety: fiber was exited to us */
				unsafe { fiber.set_start(start) };

				fiber
			}

			None => {
				trace!(target: self, "++ Creating stack for worker");

				Fiber::new_with_start(start)
			}
		}
	}

	const fn calculate_ideal(count: u64) -> u64 {
		const RATIO: u64 = 20;

		#[allow(clippy::arithmetic_side_effects)]
		(count * RATIO / 100 + 16)
	}

	/// # Safety
	/// fiber must be exited
	///
	/// This function never panics
	#[allow(clippy::missing_panics_doc)]
	pub unsafe fn exit_fiber(&self, fiber: Fiber) {
		/* we never panic with the lock */
		#[allow(clippy::unwrap_used)]
		let mut data = self.data.lock().unwrap();

		data.active = data
			.active
			.checked_sub(1)
			.expect_nounwind("Fatal error: fiber count overflow");

		let ideal = Self::calculate_ideal(data.active);

		if ideal > data.pool.len() as u64 && data.pool.try_reserve(1).is_ok() {
			trace!(target: self, "== Preserving worker stack");

			data.pool.push(fiber);
		} else {
			trace!(target: self, "-- Dropping worker stack");
		}
	}
}

impl Default for Pool {
	fn default() -> Self {
		Self::new()
	}
}
