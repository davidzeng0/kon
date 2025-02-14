use super::*;

define_enum! {
	#[repr(i32)]
	pub enum OpenAt {
		CurrentWorkingDirectory = -100
	}
}

#[must_use]
pub fn into_raw_dirfd(dirfd: Option<BorrowedFd<'_>>) -> RawFd {
	dirfd.map_or(OpenAt::CurrentWorkingDirectory as i32, |fd| fd.as_raw_fd())
}
