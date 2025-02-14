use super::signal::SignalMask;
use super::time::TimeSpec;
use super::*;
use crate::macros::wrapper_functions;

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum PollFlag {
		/// There is data to read.
		In        = 1 << 0,

		/// There is urgent data to read.
		Priority  = 1 << 1,

		/// Writing now will not block.
		Out       = 1 << 2,

		/// Error condition.
		Error     = 1 << 3,

		/// Hung up.
		HangUp    = 1 << 4,

		/// Invalid polling request.
		Invalid   = 1 << 5,

		/// Normal data may be read.
		ReadNorm  = 1 << 6,

		/// Priority data may be read.
		ReadBand  = 1 << 7,

		/// Writing now will not block.
		WriteNorm = 1 << 8,

		/// Priority data may be written.
		WriteBand = 1 << 9,

		/// Extensions for Linux
		Message   = 1 << 10,
		Remove    = 1 << 12,
		RdHangUp  = 1 << 13
	}
}

define_struct! {
	pub struct PollFd {
		pub fd: i32,
		pub events: u16,
		pub returned_events: u16
	}
}

impl PollFd {
	#[must_use]
	pub fn events(&self) -> BitFlags<PollFlag> {
		BitFlags::from_bits_truncate(self.events as u32)
	}

	#[must_use]
	pub fn returned_events(&self) -> BitFlags<PollFlag> {
		BitFlags::from_bits_truncate(self.returned_events as u32)
	}
}

#[repr(transparent)]
pub struct BorrowedPollFd<'fd> {
	poll_fd: PollFd,
	phantom: PhantomData<&'fd ()>
}

impl<'fd> BorrowedPollFd<'fd> {
	wrapper_functions! {
		inner = self.poll_fd;

		pub fn events(&self) -> BitFlags<PollFlag>;
		pub fn returned_events(&self) -> BitFlags<PollFlag>;
	}

	#[must_use]
	pub fn new(fd: BorrowedFd<'fd>, events: BitFlags<PollFlag>) -> Self {
		Self {
			poll_fd: PollFd {
				fd: fd.as_raw_fd(),
				#[allow(clippy::cast_possible_truncation)]
				events: events.bits() as u16,
				returned_events: 0
			},
			phantom: PhantomData
		}
	}
}

/// # Safety
/// `PollFd`s must be valid for this function call
#[syscall_define(Ppoll)]
pub unsafe fn ppoll(
	#[array(len = u32)] fds: &mut [PollFd], timeout: &TimeSpec, #[array] sigmask: SignalMask<'_>
) -> OsResult<u32>;

/// # Safety
/// `PollFd`s must be valid for this function call
pub unsafe fn poll_timeout(fds: &mut [PollFd], timeout: Duration) -> OsResult<u32> {
	let ts = TimeSpec::from_duration(timeout);

	/* Safety: guaranteed by caller */
	unsafe { ppoll(fds, &ts, None) }
}

pub fn poll(fds: &mut [BorrowedPollFd<'_>], timeout: Duration) -> OsResult<u32> {
	/* Safety: fds are borrowed for this function call */
	#[allow(clippy::multiple_unsafe_ops_per_block, clippy::transmute_ptr_to_ptr)]
	(unsafe { poll_timeout(transmute(fds), timeout) })
}
