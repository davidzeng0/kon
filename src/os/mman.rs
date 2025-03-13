use super::*;
use crate::impls::ResultExt;

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum Protection {
		Read      = 1 << 0,
		Write     = 1 << 1,
		Exec      = 1 << 2,
		GrowsDown = 1 << 24,
		GrowsUp   = 1 << 25
	}
}

define_enum! {
	#[repr(u32)]
	pub enum Type {
		Shared         = 1,
		Private        = 2,
		SharedValidate = 3
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum Flag {
		Fixed          = 1 << 4,
		Anonymous      = 1 << 5,
		GrowsDown      = 1 << 8,
		DenyWrite      = 1 << 11,
		Executable     = 1 << 12,
		Locked         = 1 << 13,
		NoReserve      = 1 << 14,
		Populate       = 1 << 15,
		NonBlock       = 1 << 16,
		Stack          = 1 << 17,
		HugeTLB        = 1 << 18,
		Sync           = 1 << 19,
		FixedNoReplace = 1 << 20
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum SyncFlag {
		Async      = 1 << 0,
		Invalidate = 1 << 1,
		Sync       = 1 << 2
	}
}

define_enum! {
	#[repr(u32)]
	pub enum Advice {
		Normal,
		Random,
		Sequential,
		WillNeed,
		DontNeed,
		Free = 8,
		Remove,
		DontFork,
		DoFork,
		Mergeable,
		Unmergeable,
		HugePage,
		NoHugePage,
		DontDump,
		DoDump,
		WipeOnFork,
		KeepOnFork,
		Cold,
		PageOut,
		PopulateRead,
		PopulateWrite,
		HwPoison
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum LockFlag {
		Current = 1 << 0,
		Future  = 1 << 1,
		OnFault = 1 << 2
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum RemapFlag {
		MayMove   = 1 << 0,
		Fixed     = 1 << 1,
		DontUnmap = 1 << 2
	}
}

pub struct Map<'mem> {
	addr: MutPtr<()>,
	length: usize,
	phantom: PhantomData<&'mem ()>
}

#[derive(Clone, Copy)]
pub struct Flags {
	ty: Type,
	flags: BitFlags<Flag>
}

impl Flags {
	#[must_use]
	pub fn new(ty: Type) -> Self {
		Self { ty, flags: BitFlags::default() }
	}

	#[must_use]
	pub fn flag<F>(mut self, flags: F) -> Self
	where
		F: Into<BitFlags<Flag>>
	{
		self.flags |= flags.into();
		self
	}
}

impl IntoRaw for Flags {
	type Raw = u32;

	fn into_raw(self) -> Self::Raw {
		(self.ty as u32) | self.flags.bits()
	}
}

pub struct Builder<'fd> {
	addr: Ptr<()>,
	len: usize,
	prot: BitFlags<Protection>,
	flags: Flags,
	fd: Option<BorrowedFd<'fd>>,
	off: isize
}

impl<'fd> Builder<'fd> {
	#[must_use]
	pub fn new(ty: Type, len: usize) -> Self {
		Self {
			addr: Ptr::null(),
			len,
			prot: BitFlags::default(),
			flags: Flags::new(ty),
			fd: None,
			off: 0
		}
	}

	#[must_use]
	pub fn protect<F>(mut self, protection: F) -> Self
	where
		F: Into<BitFlags<Protection>>
	{
		self.prot |= protection.into();
		self
	}

	#[must_use]
	pub fn flag<F>(mut self, flag: F) -> Self
	where
		F: Into<BitFlags<Flag>>
	{
		self.flags = self.flags.flag(flag);
		self
	}

	#[must_use]
	pub const fn fd(mut self, fd: BorrowedFd<'fd>) -> Self {
		self.fd = Some(fd);
		self
	}

	#[must_use]
	pub const fn offset(mut self, off: isize) -> Self {
		self.off = off;
		self
	}

	pub fn map(self) -> OsResult<Map<'static>> {
		Map::map(
			self.addr, self.len, self.prot, self.flags, self.fd, self.off
		)
	}

	pub fn map_raw(self) -> OsResult<MutPtr<()>> {
		mmap(
			self.addr, self.len, self.prot, self.flags, self.fd, self.off
		)
	}
}

#[syscall_define(Mmap)]
pub fn mmap(
	addr: Ptr<()>, length: usize, prot: BitFlags<Protection>, flags: Flags,
	fd: Option<BorrowedFd<'_>>, off: isize
) -> OsResult<MutPtr<()>>;

/// # Safety
/// must have ownership of the range
#[syscall_define(Munmap)]
pub unsafe fn munmap(#[array] section: RawBuf<'_>) -> OsResult<()>;

/// # Safety
/// if there are references to the range, the flags must not affect their
/// permissions
#[syscall_define(Mprotect)]
pub unsafe fn mprotect(#[array] section: RawBuf<'_>, prot: BitFlags<Protection>) -> OsResult<()>;

/// # Safety
/// section must be a valid section, returned from mmap
#[syscall_define(Msync)]
pub unsafe fn msync(#[array] section: RawBuf<'_>, flags: BitFlags<SyncFlag>) -> OsResult<()>;

/// # Safety
/// if there are references to the range, the flags must not affect their
/// permissions
#[syscall_define(Madvise)]
pub unsafe fn madvise(#[array] section: RawBuf<'_>, advice: Advice) -> OsResult<()>;

/// # Safety
/// section must be a valid section, returned from mmap
#[syscall_define(Mlock)]
pub unsafe fn mlock(#[array] section: RawBuf<'_>) -> OsResult<()>;

/// # Safety
/// section must be a valid section, returned from mmap
#[syscall_define(Mlock)]
pub unsafe fn mlock2(#[array] section: RawBuf<'_>, flags: BitFlags<LockFlag>) -> OsResult<()>;

/// # Safety
/// section must be a valid section, returned from mmap
#[syscall_define(Munlock)]
pub unsafe fn munlock(#[array] section: RawBuf<'_>) -> OsResult<()>;

#[syscall_define(Mlockall)]
pub fn mlock_all(flags: BitFlags<LockFlag>) -> OsResult<()>;

/// # Safety
/// if there are no mappings that require locking
#[syscall_define(Munlockall)]
pub unsafe fn munlock_all() -> OsResult<()>;

/// # Safety
/// section must be a valid section, returned from mmap
/// the section must be owned
#[syscall_define(Mremap)]
pub unsafe fn mremap(
	#[array] section: RawBuf<'_>, new_length: usize, flags: BitFlags<RemapFlag>,
	new_address: Ptr<()>
) -> OsResult<MutPtr<()>>;

impl Default for Map<'static> {
	fn default() -> Self {
		Self::new()
	}
}

impl<'mem> Map<'mem> {
	#[must_use]
	pub const fn new() -> Self {
		Self {
			addr: MutPtr::null(),
			length: 0,
			phantom: PhantomData
		}
	}

	#[allow(clippy::self_named_constructors)]
	pub fn map(
		addr: Ptr<()>, length: usize, prot: BitFlags<Protection>, flags: Flags,
		fd: Option<BorrowedFd<'_>>, off: isize
	) -> OsResult<Map<'static>> {
		let addr = mmap(addr, length, prot, flags, fd, off)?;

		Ok(Map { addr, length, phantom: PhantomData })
	}

	#[must_use]
	pub const fn as_ptr(&self) -> MutPtr<()> {
		self.addr
	}

	#[must_use]
	#[allow(clippy::len_without_is_empty)]
	pub const fn len(&self) -> usize {
		self.length
	}

	#[must_use]
	pub const fn section(&self) -> RawBuf<'mem> {
		RawBuf::from_parts(self.addr.cast_const(), self.length)
	}

	/// # Safety
	/// see `mprotect`
	pub unsafe fn protect(&self, prot: BitFlags<Protection>) -> OsResult<()> {
		/* Safety: guaranteed by caller */
		unsafe { mprotect(self.section(), prot) }
	}

	/// # Safety
	/// see `msync`
	pub unsafe fn sync(&self, flags: BitFlags<SyncFlag>) -> OsResult<()> {
		/* Safety: guaranteed by caller */
		unsafe { msync(self.section(), flags) }
	}

	/// # Safety
	/// see `madvise`
	pub unsafe fn advise(&self, advice: Advice) -> OsResult<()> {
		/* Safety: guaranteed by caller */
		unsafe { madvise(self.section(), advice) }
	}

	/// # Safety
	/// see `mlock`
	pub unsafe fn lock(&self) -> OsResult<()> {
		/* Safety: guaranteed by caller */
		unsafe { mlock(self.section()) }
	}

	/// # Safety
	/// see `munlock`
	pub unsafe fn unlock(&self) -> OsResult<()> {
		/* Safety: guaranteed by caller */
		unsafe { munlock(self.section()) }
	}
}

impl Drop for Map<'_> {
	fn drop(&mut self) {
		if self.addr.is_null() {
			return;
		}

		/* Safety: owner dropped us, so we own the range */
		unsafe { munmap(self.section()) }.expect_nounwind("Failed to unmap memory");
	}
}
