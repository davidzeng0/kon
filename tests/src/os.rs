use std::mem::transmute;
use std::os::fd::{FromRawFd, OwnedFd};
use std::time::Duration;

use xx_core::os::error::{result_from_int, result_from_ptr, OsError};
use xx_core::os::mman::{Advice, Flag, Flags, Map, Protection, Type};
use xx_core::os::poll::{poll_timeout, PollFd, PollFlag};
use xx_core::os::resource::{get_rlimit, Resource};
use xx_core::os::time::{nanotime, ClockId};
use xx_core::os::unistd::close;
use xx_core::pointer::{MutPtr, Ptr};

#[test]
fn test_close_inval() {
	close(unsafe { transmute(-2) }).unwrap_err();
	close(unsafe { OwnedFd::from_raw_fd(-2) }).unwrap_err();
	close(unsafe { OwnedFd::from_raw_fd(-20) }).unwrap_err();
	close(unsafe { OwnedFd::from_raw_fd(555) }).unwrap_err();
}

#[test]
fn test_time() {
	assert!(nanotime(ClockId::Monotonic).unwrap() > 0);
}

#[test]
fn test_rlimit() {
	assert!(get_rlimit(Resource::Stack).unwrap().current > 0);
}

#[test]
fn test_poll() {
	let mut fds = [PollFd {
		fd: 1,
		events: PollFlag::Out as u16,
		returned_events: 0
	}];

	unsafe { poll_timeout(&mut fds, Duration::ZERO).unwrap() };

	assert!(fds[0].returned_events().intersects(PollFlag::Out));
}

#[test]
fn test_mmap() {
	let mem = Map::map(
		Ptr::from_addr(0x12345678000),
		16384,
		Protection::Read.into(),
		Flags::new(Type::Private).flag(Flag::Anonymous),
		None,
		0
	)
	.unwrap();

	assert_eq!(mem.len(), 16384);
	assert_eq!(mem.as_ptr(), MutPtr::from_addr(0x12345678000));

	unsafe {
		mem.protect(Protection::Write.into()).unwrap();
		mem.advise(Advice::Random).unwrap();
		mem.lock().unwrap();
		mem.unlock().unwrap();
	}

	drop(mem);
}

#[test]
fn test_error() {
	result_from_int(-2).unwrap_err();
	result_from_int(0).unwrap();
	result_from_int(2).unwrap();
	result_from_ptr(-2).unwrap_err();
	result_from_ptr(0).unwrap();
	result_from_ptr(2).unwrap();
	result_from_ptr(-4095).unwrap_err();
	result_from_ptr(-4096).unwrap();
	result_from_ptr(isize::MIN).unwrap();
	result_from_ptr(isize::MAX).unwrap();
	assert_eq!(OsError::from(2), OsError::NoEnt);
}
