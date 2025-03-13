use std::hint::spin_loop;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::yield_now;

/// A spin lock for when the critical section is short and predictable
pub struct SpinLock(AtomicBool);

impl SpinLock {
	#[must_use]
	pub const fn new() -> Self {
		Self(AtomicBool::new(false))
	}

	pub fn try_lock(&self) -> bool {
		self.0
			.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
			.is_ok()
	}

	pub fn lock(&self) {
		if !self.try_lock() {
			self.lock_contended();
		}
	}

	pub fn try_spin_lock(&self) -> bool {
		for _ in 0..97 {
			let locked = self.0.load(Ordering::Relaxed);

			if !locked {
				return self.try_lock();
			}

			spin_loop();
		}

		false
	}

	#[cold]
	fn lock_contended(&self) {
		while !self.try_spin_lock() {
			yield_now();
		}
	}

	pub fn unlock(&self) {
		self.0.store(false, Ordering::Release);
	}
}

impl Default for SpinLock {
	fn default() -> Self {
		Self::new()
	}
}
