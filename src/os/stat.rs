use super::dirent::FileType;
use super::fcntl::AtFlag;
use super::openat::into_raw_dirfd;
use super::*;

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum StatxMask {
		Type                = 1 << 0,
		Mode                = 1 << 1,
		HardLinksCount      = 1 << 2,
		UserId              = 1 << 3,
		GroupId             = 1 << 4,
		AccessTime          = 1 << 5,
		ModifiedTime        = 1 << 6,
		AttributeChangeTime = 1 << 7,
		Inode               = 1 << 8,
		Size                = 1 << 9,
		Blocks              = 1 << 10,
		CreationTime        = 1 << 11,
		MountId             = 1 << 12,
		DirectIoAlign       = 1 << 13
	}
}

#[allow(non_upper_case_globals)]
impl StatxMask {
	pub const All: u32 = 0x0fff;
	pub const BasicStats: u32 = 0x07ff;
}

define_struct! {
	pub struct StatxTimestamp {
		pub sec: i64,
		pub nanos: u32,
		pub resv: [i32; 1]
	}
}

define_struct! {
	pub struct Statx {
		pub mask: u32,
		pub block_size: u32,
		pub attributes: u64,
		pub hard_links_count: u32,
		pub user_id: u32,
		pub group_id: u32,
		pub mode: u16,
		pub resv: [u16; 1],
		pub inode: u64,
		pub size: u64,
		pub blocks: u64,
		pub attributes_mask: u64,

		pub access_time: StatxTimestamp,
		pub creation_time: StatxTimestamp,
		pub attribute_change_time: StatxTimestamp,
		pub modified_time: StatxTimestamp,

		pub rdev_major: u32,
		pub rdev_minor: u32,
		pub dev_major: u32,
		pub dev_minor: u32,

		pub mount_id: u64,
		pub direct_io_mem_align: u32,
		pub direct_io_offset_align: u32,

		pub resv1: [u64; 12]
	}
}

impl Statx {
	#[must_use]
	pub fn mask(&self) -> BitFlags<StatxMask> {
		BitFlags::from_bits_truncate(self.mask)
	}

	#[must_use]
	pub fn file_type(&self) -> Option<FileType> {
		if !self.mask().intersects(StatxMask::Mode) {
			return None;
		}

		FileType::from_u16(self.mode >> 12)
	}
}

pub mod internal {
	use super::*;

	#[syscall_define(Statx)]
	pub fn statx(
		dirfd: RawFd, pathname: &CStr, flags: u32, mask: u32, statx: &mut Statx
	) -> OsResult<()>;
}

pub fn statx(
	dirfd: Option<BorrowedFd<'_>>, pathname: &CStr, flags: u32, mask: u32, statx: &mut Statx
) -> OsResult<()> {
	let dirfd = into_raw_dirfd(dirfd);

	internal::statx(dirfd, pathname, flags, mask, statx)
}

pub fn statx_fd(fd: BorrowedFd<'_>, flags: u32, mask: u32, statx: &mut Statx) -> OsResult<()> {
	internal::statx(
		fd.as_raw_fd(),
		c"",
		flags | (AtFlag::EmptyPath as u32),
		mask,
		statx
	)
}
