use super::error::*;
use super::fcntl::OpenFlag;
use super::stat::{Statx, *};
use super::unistd::open;
use super::*;

define_enum! {
	#[repr(u8)]
	pub enum FileType {
		/// The file type could not be determined
		Unknown,

		/// This is a named pipe
		Fifo,

		/// This is a character device
		Character,

		/// This is a directory
		Directory  = 4,

		/// This is a block device
		Block  = 6,

		/// This is a regular file
		Regular  = 8,

		/// This is a symbolic link
		Link  = 10,

		/// This is a UNIX domain socket
		Socket = 12,

		Wht  = 14
	}
}

define_struct! {
	pub struct DirentDef<T: ?Sized> {
		pub ino: u64,
		pub off: i64,
		pub reclen: u16,
		pub ty: u8,
		pub name: T
	}
}

define_struct! {
	pub struct DirentCompatDef<T: ?Sized> {
		pub ino: u32,
		pub off: u32,
		pub reclen: u16,
		pub name: T
	}
}

impl<T: ?Sized> DirentDef<T> {
	pub fn file_type(&self) -> Option<FileType> {
		FileType::from_u8(self.ty)
	}
}

pub type Dirent = DirentDef<[u8]>;
pub type DirentCompat = DirentCompatDef<[u8]>;

#[syscall_define(Getdents)]
pub unsafe fn getdents(
	fd: BorrowedFd<'_>, #[array(len = u32)] buf: MutRawBuf<'_>
) -> OsResult<usize>;

#[syscall_define(Getdents64)]
pub unsafe fn getdents64(
	fd: BorrowedFd<'_>, #[array(len = u32)] buf: MutRawBuf<'_>
) -> OsResult<usize>;

pub struct DirEnts {
	buf: Box<[u8]>,
	offset: usize,
	len: usize,
	eof: bool
}

impl DirEnts {
	#[must_use]
	pub fn new(size: usize) -> Self {
		Self {
			buf: vec![0u8; size].into_boxed_slice(),
			offset: 0,
			len: 0,
			eof: false
		}
	}

	#[must_use]
	pub fn new_from_block_size(size: usize) -> Self {
		/* 32 KiB */
		const MIN_SIZE: usize = 0x0000_8000;

		/* 1 MiB */
		const MAX_SIZE: usize = 0x0010_0000;

		Self::new(size.clamp(MIN_SIZE, MAX_SIZE))
	}

	pub fn read_from_fd(&mut self, fd: BorrowedFd<'_>) -> OsResult<()> {
		self.offset = 0;
		self.len = 0;

		if self.eof {
			return Ok(());
		}

		/* Safety: valid buffer */
		let count = match unsafe { getdents64(fd, (&mut self.buf[..]).into()) } {
			Ok(n) => n,
			Err(OsError::NoEnt) => 0,
			Err(err) => return Err(err)
		};

		if count == 0 {
			self.eof = true;
		}

		self.len = count;

		Ok(())
	}

	#[allow(clippy::multiple_unsafe_ops_per_block)]
	pub fn next_entry(&mut self) -> Option<DirentDef<&CStr>> {
		if !self.has_next_cached() {
			return None;
		}

		/* Safety: ptr and offset are always valid */
		let header = unsafe {
			ptr!(self.buf.as_ptr())
				.add(self.offset)
				.cast::<DirentDef<u8>>()
				.as_ref()
		};

		/* Safety: string is null terminated */
		let name = unsafe { CStr::from_ptr(ptr!(&header.name).as_ptr().cast()) };

		#[allow(clippy::arithmetic_side_effects)]
		(self.offset += header.reclen as usize);

		Some(DirentDef {
			ino: header.ino,
			off: header.off,
			reclen: header.reclen,
			ty: header.ty,
			name
		})
	}

	#[must_use]
	pub const fn has_next_cached(&self) -> bool {
		self.offset < self.len
	}

	#[must_use]
	pub const fn is_eof(&self) -> bool {
		self.eof
	}
}

pub struct ReadDir {
	fd: OwnedFd,
	entries: DirEnts
}

impl ReadDir {
	#[allow(clippy::impl_trait_in_params)]
	pub fn open(path: impl AsRef<Path>) -> Result<Self> {
		let flags = make_bitflags!(OpenFlag::{
			Directory | LargeFile | CloseOnExec | NonBlock
		});

		let fd = with_path_as_cstr(path, |path| open(path, flags.bits(), 0).map_err(Into::into))?;
		let mut statx = Statx::default();

		statx_fd(fd.as_fd(), 0, 0, &mut statx)?;

		if statx.file_type() != Some(FileType::Directory) {
			return Err(OsError::NotDir.into());
		}

		let entries = DirEnts::new_from_block_size(statx.block_size as usize);

		Ok(Self { fd, entries })
	}

	pub fn next_entry(&mut self) -> OsResult<Option<DirentDef<&CStr>>> {
		if !self.entries.has_next_cached() {
			self.entries.read_from_fd(self.fd.as_fd())?;
		}

		Ok(self.entries.next_entry())
	}
}
