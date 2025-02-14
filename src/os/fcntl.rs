use super::*;

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum OpenFlag {
		WriteOnly   = 1 << 0,
		ReadWrite   = 1 << 1,

		/// If the file does not exist, create it as a regular file
		Create      = 1 << 6,

		/// Ensure that this call creates the file: if this flag is specified
		/// in conjunction with Create, and pathname already exists, then
		/// open() fails with the error OsError::Exist.
		Excl        = 1 << 7,
		NocTTY      = 1 << 8,

		/// If the file already exists and is a regular file and the access
		/// mode allows writing (i.e., is ReadWrite or WriteOnly) it will be
		/// truncated to length 0.
		Truncate    = 1 << 9,
		Append      = 1 << 10,
		NonBlock    = 1 << 11,
		DataSync    = 1 << 12,
		Async       = 1 << 13,

		/// Try to minimize cache effects of the I/O to and from this file. In
		/// general this will degrade performance, but it is useful in
		/// special situations, such as when applications do their own
		/// caching. File I/O is done directly to/from user-space buffers.
		Direct      = 1 << 14,
		LargeFile   = 1 << 15,
		Directory   = 1 << 16,
		NoFollow    = 1 << 17,
		NoATime     = 1 << 18,

		/// Enable the close-on-exec flag for the new file descriptor.
		CloseOnExec = 1 << 19,
		Path        = 1 << 21
	}
}

#[allow(non_upper_case_globals)]
impl OpenFlag {
	pub const AccMode: u32 = 0x03;
	pub const ReadOnly: u32 = 0x00;
	pub const Sync: u32 = 0x0010_1000;
	pub const TempFile: u32 = 0x0040_0000 | Self::Directory as u32;
}

pub trait OpenFlagExtensions {
	fn access_mode(self) -> BitFlags<OpenFlag>;

	fn access(self, read: bool, write: bool) -> BitFlags<OpenFlag>;

	fn temporary(self) -> BitFlags<OpenFlag>;
}

impl OpenFlagExtensions for BitFlags<OpenFlag> {
	#[allow(clippy::unwrap_used)]
	fn access_mode(self) -> BitFlags<OpenFlag> {
		self & BitFlags::from_bits(OpenFlag::AccMode).unwrap()
	}

	fn access(mut self, read: bool, write: bool) -> BitFlags<OpenFlag> {
		self.remove(self.access_mode());

		if write {
			self |= OpenFlag::WriteOnly;

			if read {
				self |= OpenFlag::ReadWrite;
			}
		}

		self
	}

	#[allow(clippy::unwrap_used)]
	fn temporary(self) -> BitFlags<OpenFlag> {
		self | BitFlags::from_bits(OpenFlag::TempFile).unwrap()
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum AtFlag {
		SymlinkNoFollow = 1 << 8,
		EAccess = 1 << 9,
		SymlinkFollow = 1 << 10,
		NoAutoMount = 1 << 11,
		EmptyPath = 1 << 12,
		ForceSync = 1 << 13,
		DontSync = 1 << 14,
		Recursive = 1 << 15
	}
}

#[allow(non_upper_case_globals)]
impl AtFlag {
	pub const RemoveDir: Self = Self::EAccess;
}
