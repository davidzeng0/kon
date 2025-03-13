use super::error::*;
use super::openat::into_raw_dirfd;
use super::openat2::OpenHow;
use super::*;

pub mod sysconf;
pub use sysconf::*;

pub mod internal {

	use super::*;

	#[syscall_define(Openat)]
	pub fn openat(dirfd: RawFd, filename: &CStr, flags: u32, mode: u32) -> OsResult<OwnedFd>;

	#[syscall_define(Openat2)]
	pub fn openat2(dirfd: RawFd, filename: &CStr, how: &OpenHow, size: usize) -> OsResult<OwnedFd>;
}

#[syscall_define(Open)]
pub fn open(filename: &CStr, flags: u32, mode: u32) -> OsResult<OwnedFd>;

pub fn openat(
	dirfd: Option<BorrowedFd<'_>>, filename: &CStr, flags: u32, mode: u32
) -> OsResult<OwnedFd> {
	let dirfd = into_raw_dirfd(dirfd);

	internal::openat(dirfd, filename, flags, mode)
}

pub fn openat2(dirfd: Option<BorrowedFd<'_>>, filename: &CStr, how: &OpenHow) -> OsResult<OwnedFd> {
	let dirfd = into_raw_dirfd(dirfd);

	internal::openat2(dirfd, filename, how, size_of::<OpenHow>())
}

#[syscall_define(Close)]
pub fn close(fd: OwnedFd) -> OsResult<()>;

#[syscall_define(Read)]
pub fn read(fd: BorrowedFd<'_>, #[array] buf: MutRawBuf<'_>) -> OsResult<()>;

#[syscall_define(Write)]
pub fn write(fd: BorrowedFd<'_>, #[array] buf: RawBuf<'_>) -> OsResult<()>;
