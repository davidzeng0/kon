use super::fcntl::OpenFlag;
use super::poll::*;
use super::unistd::*;
use super::*;

define_enum! {
	#[repr(u32)]
	#[bitflags]
	pub enum CreateFlag {
		Semaphore   = 0x1,
		NonBlock    = OpenFlag::NonBlock as u32,
		CloseOnExec = OpenFlag::CloseOnExec as u32
	}
}

#[syscall_define(Eventfd2)]
pub fn eventfd2(init: u32, flags: BitFlags<CreateFlag>) -> OsResult<OwnedFd>;

pub struct EventFd(OwnedFd);

impl EventFd {
	pub fn new(flags: BitFlags<CreateFlag>) -> OsResult<Self> {
		eventfd2(0, flags).map(Self)
	}

	pub fn read(&self) -> OsResult<u64> {
		let mut bytes = 0u64.to_ne_bytes();

		read(self.0.as_fd(), (&mut bytes).into())?;

		Ok(u64::from_ne_bytes(bytes))
	}

	pub fn write(&self, value: u64) -> OsResult<()> {
		let bytes = value.to_ne_bytes();

		write(self.0.as_fd(), (&bytes).into())?;

		Ok(())
	}

	#[must_use]
	pub fn fd(&self) -> BorrowedFd<'_> {
		self.0.as_fd()
	}

	pub fn poll(
		&self, events: BitFlags<PollFlag>, timeout: Duration
	) -> OsResult<BitFlags<PollFlag>> {
		let mut fds = [BorrowedPollFd::new(self.fd(), events)];

		Ok(if poll(&mut fds, timeout)? == 0 {
			BitFlags::empty()
		} else {
			fds[0].returned_events()
		})
	}
}
