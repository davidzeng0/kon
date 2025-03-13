use super::*;
use crate::macros::strings;

#[non_exhaustive]
#[strings]
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum ErrorKind {
	#[string = "Entity not found"]
	NotFound,

	#[string = "Permission denied"]
	PermissionDenied,

	#[string = "Connection refused"]
	ConnectionRefused,

	#[string = "Connection reset"]
	ConnectionReset,

	#[string = "Host unreachable"]
	HostUnreachable,

	#[string = "Network unreachable"]
	NetworkUnreachable,

	#[string = "Connection aborted"]
	ConnectionAborted,

	#[string = "Not connected"]
	NotConnected,

	#[string = "Address in use"]
	AddrInUse,

	#[string = "Address not available"]
	AddrNotAvailable,

	#[string = "Network down"]
	NetworkDown,

	#[string = "Broken pipe"]
	BrokenPipe,

	#[string = "Entity already exists"]
	AlreadyExists,

	#[string = "Operation would block"]
	WouldBlock,

	#[string = "Not a directory"]
	NotADirectory,

	#[string = "Is a directory"]
	IsADirectory,

	#[string = "Directory not empty"]
	DirectoryNotEmpty,

	#[string = "Read-only file system"]
	ReadOnlyFilesystem,

	#[string = "Too many levels of symbolic links"]
	FileSystemLoop,

	#[string = "Stale network file handle"]
	StaleNetworkFileHandle,

	#[string = "Invalid argument"]
	InvalidInput,

	#[string = "Invalid data"]
	InvalidData,

	#[string = "Operation timed out"]
	TimedOut,

	#[string = "Write EOF"]
	WriteZero,

	#[string = "Storage full"]
	StorageFull,

	#[string = "Not seekable"]
	NotSeekable,

	#[string = "Disk quota exceeded"]
	DiskQuotaExceeded,

	#[string = "File too large"]
	FileTooLarge,

	#[string = "Resource busy"]
	ResourceBusy,

	#[string = "Executable file busy"]
	ExecutableFileBusy,

	#[string = "Dead lock"]
	Deadlock,

	#[string = "Invalid cross-device link"]
	CrossesDevices,

	#[string = "Too many links"]
	TooManyLinks,

	#[string = "Invalid file name"]
	InvalidFilename,

	#[string = "Argument list too long"]
	ArgumentListTooLong,

	#[string = "Operation interrupted"]
	Interrupted,

	#[string = "Unsupported"]
	Unsupported,

	#[string = "Unexpected end of file"]
	UnexpectedEof,

	#[string = "Out of memory"]
	OutOfMemory,

	#[string = "Overflow occurred"]
	Overflow,

	#[string = "Resource is shutdown"]
	Shutdown,

	#[string = "Formatter error"]
	FormatterError,

	#[string = "Not implemented"]
	Unimplemented,

	#[string = "Operation already in progress"]
	AlreadyInProgress,

	#[string = "No data"]
	NoData,

	#[string = "Other"]
	Other,

	#[string = "Uncategorized"]
	Uncategorized
}

impl Display for ErrorKind {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
		fmt.write_str(self.as_str())
	}
}

#[cfg(feature = "os")]
impl From<OsError> for ErrorKind {
	fn from(value: OsError) -> Self {
		use ErrorKind::*;

		match value {
			OsError::TooBig => ArgumentListTooLong,
			OsError::AddrInUse => AddrInUse,
			OsError::AddrNotAvail => AddrNotAvailable,
			OsError::Busy => ResourceBusy,
			OsError::ConnAborted => ConnectionAborted,
			OsError::ConnRefused => ConnectionRefused,
			OsError::ConnReset => ConnectionReset,
			OsError::Deadlock => Deadlock,
			OsError::DQuot => DiskQuotaExceeded,
			OsError::Exist => AlreadyExists,
			OsError::FBig => FileTooLarge,
			OsError::HostUnreach => HostUnreachable,
			OsError::Intr => Interrupted,
			OsError::Inval => InvalidInput,
			OsError::IsDir => IsADirectory,
			OsError::Loop => FileSystemLoop,
			OsError::NoEnt => NotFound,
			OsError::NoMem => OutOfMemory,
			OsError::NoSpc => StorageFull,
			OsError::NoSys => Unsupported,
			OsError::MLink => TooManyLinks,
			OsError::NameTooLong => InvalidFilename,
			OsError::NetDown => NetworkDown,
			OsError::NetUnreach => NetworkUnreachable,
			OsError::NotConn => NotConnected,
			OsError::NotDir => NotADirectory,
			OsError::NotEmpty => DirectoryNotEmpty,
			OsError::Pipe => BrokenPipe,
			OsError::Rofs => ReadOnlyFilesystem,
			OsError::SPipe => NotSeekable,
			OsError::Stale => StaleNetworkFileHandle,
			OsError::TimedOut => TimedOut,
			OsError::TxtBsy => ExecutableFileBusy,
			OsError::XDev => CrossesDevices,
			OsError::Acces | OsError::Perm => PermissionDenied,
			OsError::Canceled => Interrupted,

			code if code == OsError::Again || code == OsError::WouldBlock => WouldBlock,

			_ => Uncategorized
		}
	}
}

#[cfg(not(feature = "os"))]
impl From<OsError> for ErrorKind {
	fn from(_: OsError) -> Self {
		Self::Other
	}
}

impl From<io::ErrorKind> for ErrorKind {
	fn from(value: io::ErrorKind) -> Self {
		use io::ErrorKind::*;

		match value {
			NotFound => Self::NotFound,
			PermissionDenied => Self::PermissionDenied,
			ConnectionRefused => Self::ConnectionRefused,
			ConnectionReset => Self::ConnectionReset,
			ConnectionAborted => Self::ConnectionAborted,
			NotConnected => Self::NotConnected,
			AddrInUse => Self::AddrInUse,
			AddrNotAvailable => Self::AddrNotAvailable,
			BrokenPipe => Self::BrokenPipe,
			AlreadyExists => Self::AlreadyExists,
			WouldBlock => Self::WouldBlock,
			InvalidInput => Self::InvalidInput,
			InvalidData => Self::InvalidData,
			TimedOut => Self::TimedOut,
			WriteZero => Self::WriteZero,
			Interrupted => Self::Interrupted,
			Unsupported => Self::Unsupported,
			UnexpectedEof => Self::UnexpectedEof,
			OutOfMemory => Self::OutOfMemory,
			_ => Self::Other
		}
	}
}
