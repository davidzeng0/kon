use std::sync::atomic::{AtomicU32, Ordering};

use super::error::OsError;
use super::time::TimeSpec;
use super::*;

define_enum! {
	#[repr(i32)]
	pub enum FutexOp {
		Wait = 0,
		Wake,
		Fd,
		Requeue,
		CmpRequeue,
		WakeOp,
		LockPi,
		UnlockPi,
		TryLockPi,
		WaitBitset,
		WakeBitset,
		WaitRequeuePi,
		CmpRequeuePi,
		LockPi2,
	}
}

#[allow(non_upper_case_globals)]
impl FutexOp {
	pub const ClockRealtime: i32 = 1 << 8;
	pub const PrivateFlag: i32 = 1 << 7;
}

#[syscall_define(Futex)]
pub unsafe fn futex(
	addr: MutPtr<u32>, op: i32, value: u32, time: Option<&TimeSpec>, addr2: MutPtr<u32>,
	value3: u32
) -> OsResult<u64>;

#[repr(u32)]
enum State {
	Parked = 0,
	Idle,
	Notified
}

pub struct Notify {
	state: AtomicU32
}

impl Notify {
	#[must_use]
	pub const fn new() -> Self {
		Self { state: AtomicU32::new(State::Idle as u32) }
	}

	/// # Safety
	/// this `Notify` must be pinned
	pub unsafe fn wait(&self) -> OsResult<bool> {
		if self.state.fetch_sub(1, Ordering::Relaxed) == State::Notified as u32 {
			return Ok(true);
		}

		/* Safety: futex is pinned */
		let result = unsafe {
			futex(
				self.state.as_ptr().into(),
				FutexOp::Wait as i32 | FutexOp::PrivateFlag,
				State::Parked as u32,
				None,
				MutPtr::null(),
				0
			)
		};

		let state = self.state.swap(State::Idle as u32, Ordering::Relaxed);

		match result {
			Ok(_) | Err(OsError::Again | OsError::Intr) => Ok(state == State::Notified as u32),
			Err(err) => Err(err)
		}
	}

	/// # Safety
	/// this `Notify` must be pinned
	pub unsafe fn notify(&self) -> Result<()> {
		if self.state.swap(State::Notified as u32, Ordering::Relaxed) != State::Parked as u32 {
			return Ok(());
		}

		/* Safety: futex is pinned */
		unsafe {
			futex(
				self.state.as_ptr().into(),
				FutexOp::Wake as i32 | FutexOp::PrivateFlag,
				1,
				None,
				MutPtr::null(),
				0
			)?;
		}

		Ok(())
	}
}

impl Pin for Notify {}

impl Default for Notify {
	fn default() -> Self {
		Self::new()
	}
}
