use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LockResult, PoisonError};
use std::thread::panicking;

pub struct PoisonGuard<'a> {
	flag: &'a PoisonFlag,
	#[cfg(panic = "unwind")]
	panicking: bool
}

impl<'a> PoisonGuard<'a> {
	fn new(flag: &'a PoisonFlag) -> Self {
		Self {
			flag,
			#[cfg(panic = "unwind")]
			panicking: panicking()
		}
	}

	pub fn finish(&self) {
		#[cfg(panic = "unwind")]
		if !self.panicking && panicking() {
			self.flag.failed.store(true, Ordering::Relaxed);
		}
	}
}

pub struct PoisonFlag {
	#[cfg(panic = "unwind")]
	failed: AtomicBool
}

impl PoisonFlag {
	#[must_use]
	pub const fn new() -> Self {
		Self {
			#[cfg(panic = "unwind")]
			failed: AtomicBool::new(false)
		}
	}

	pub fn guard(&self) -> PoisonGuard<'_> {
		PoisonGuard::new(self)
	}

	pub fn get(&self) -> bool {
		#[cfg(panic = "unwind")]
		return self.failed.load(Ordering::Relaxed);

		#[cfg(not(panic = "unwind"))]
		false
	}

	pub fn clear(&self) {
		#[cfg(panic = "unwind")]
		self.failed.store(false, Ordering::Relaxed);
	}

	pub fn map<G>(&self, guard: G) -> LockResult<G> {
		match self.get() {
			false => Ok(guard),
			true => Err(PoisonError::new(guard))
		}
	}
}

impl Default for PoisonFlag {
	fn default() -> Self {
		Self::new()
	}
}
