use super::fcntl::OpenFlag;
use super::poll::PollFlag::*;
use super::signal::*;
use super::time::TimeSpec;
use super::*;

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum PollFlag {
		/// There is data to read.
		In            = In as u32,

		/// There is urgent data to read.
		Priority      = Priority as u32,

		/// Writing now will not block.
		Out           = Out as u32,

		/// Error condition.
		Error         = Error as u32,

		/// Hung up.
		HangUp        = HangUp as u32,

		/// Normal data may be read.
		ReadNorm      = ReadNorm as u32,

		/// Priority data may be read.
		ReadBand      = ReadBand as u32,

		/// Writing now will not block.
		WriteNorm     = WriteNorm as u32,

		/// Priority data may be written.
		WriteBand     = WriteBand as u32,

		Message       = Message as u32,

		RdHangUp      = RdHangUp as u32,

		Exclusive     = 1 << 28,
		WakeUp        = 1 << 29,
		OneShot       = 1 << 30,
		EdgeTriggered = 1 << 31
	}
}

define_enum! {
	#[repr(u32)]
	pub enum ControlOp {
		/// Add an entry to the interest list of the epoll file descriptor
		Add = 1,

		/// Remove (deregister) the target file descriptor fd from the interest list.
		Del,

		/// Change the settings associated with fd in the interest list to the new settings
		Mod
	}
}

define_enum! {
	#[repr(u32)]
	#[bitflags]
	pub enum CreateFlag {
		CloseOnExec = OpenFlag::CloseOnExec as u32
	}
}

define_struct! {
	#[repr(packed)]
	pub struct Event {
		pub events: u32,
		pub data: u64
	}
}

pub fn epoll_create(_size: u32) -> OsResult<OwnedFd> {
	epoll_create1(BitFlags::default())
}

#[syscall_define(EpollCreate1)]
pub fn epoll_create1(flags: BitFlags<CreateFlag>) -> OsResult<OwnedFd>;

#[syscall_define(EpollCtl)]
pub fn epoll_ctl(
	epfd: BorrowedFd<'_>, op: ControlOp, fd: BorrowedFd<'_>, event: &mut Event
) -> OsResult<()>;

pub fn epoll_wait(fd: BorrowedFd<'_>, events: &mut [Event], timeout: i32) -> OsResult<u32> {
	epoll_pwait(fd, events, timeout, None)
}

#[syscall_define(EpollPwait)]
pub fn epoll_pwait(
	fd: BorrowedFd<'_>, #[array(len = i32)] events: &mut [Event], timeout: i32,
	#[array] sigmask: SignalMask<'_>
) -> OsResult<u32>;

#[syscall_define(EpollPwait2)]
pub fn epoll_pwait2(
	fd: BorrowedFd<'_>, #[array(len = i32)] events: &mut [Event], timeout: &TimeSpec,
	#[array] sigmask: SignalMask<'_>
) -> OsResult<u32>;

pub struct EventPoll(OwnedFd);

impl EventPoll {
	pub fn new(flags: BitFlags<CreateFlag>) -> OsResult<Self> {
		epoll_create1(flags).map(Self)
	}

	pub fn ctl(&self, op: ControlOp, fd: BorrowedFd<'_>, event: &mut Event) -> OsResult<()> {
		epoll_ctl(self.0.as_fd(), op, fd, event)
	}

	pub fn wait(&self, events: &mut [Event], timeout: Duration) -> OsResult<u32> {
		let ts = TimeSpec::from_duration(timeout);

		epoll_pwait2(self.0.as_fd(), events, &ts, None)
	}
}
