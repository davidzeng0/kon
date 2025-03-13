use super::*;
use crate::macros::strings;

#[strings]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, FromPrimitive)]
#[repr(i16)]
pub enum OsError {
	#[string = "Unknown error"]
	Unknown = -1,

	#[string = "Success"]
	Ok      = 0,

	#[string = "Operation not permitted"]
	Perm,

	#[string = "No such file or directory"]
	NoEnt,

	#[string = "No such process"]
	Srch,

	#[string = "Interrupted system call"]
	Intr,

	#[string = "Input/output error"]
	Io,

	#[string = "No such device or address"]
	Nxio,

	#[string = "Argument list too long"]
	TooBig,

	#[string = "Exec format error"]
	NoExec,

	#[string = "Bad file descriptor"]
	BadF,

	#[string = "No child processes"]
	Child,

	#[string = "Resource temporarily unavailable"]
	Again,

	#[string = "Cannot allocate memory"]
	NoMem,

	#[string = "Permission denied"]
	Acces,

	#[string = "Bad address"]
	Fault,

	#[string = "Block device required"]
	NotBlk,

	#[string = "Device or resource busy"]
	Busy,

	#[string = "File exists"]
	Exist,

	#[string = "Invalid cross-device link"]
	XDev,

	#[string = "No such device"]
	NoDev,

	#[string = "Not a directory"]
	NotDir,

	#[string = "Is a directory"]
	IsDir,

	#[string = "Invalid argument"]
	Inval,

	#[string = "Too many open files in system"]
	NFile,

	#[string = "Too many open files"]
	MFile,

	#[string = "Inappropriate ioctl for device"]
	NotTy,

	#[string = "Text file busy"]
	TxtBsy,

	#[string = "File too large"]
	FBig,

	#[string = "No space left on device"]
	NoSpc,

	#[string = "Illegal seek"]
	SPipe,

	#[string = "Read-only file system"]
	Rofs,

	#[string = "Too many links"]
	MLink,

	#[string = "Broken pipe"]
	Pipe,

	#[string = "Numerical argument out of domain"]
	Dom,

	#[string = "Numerical result out of range"]
	Range,

	#[string = "Resource deadlock avoided"]
	Deadlock,

	#[string = "File name too long"]
	NameTooLong,

	#[string = "No locks available"]
	NoLck,

	#[string = "Not implemented"]
	NoSys,

	#[string = "Directory not empty"]
	NotEmpty,

	#[string = "Too many levels of symbolic links"]
	Loop,

	#[string = "No message of desired type"]
	Nomsg   = 42,

	#[string = "Identifier removed"]
	Idrm,

	#[string = "Channel number out of range"]
	Chrng,

	#[string = "Level 2 not synchronized"]
	L2nsync,

	#[string = "Level 3 halted"]
	L3hlt,

	#[string = "Level 3 reset"]
	L3rst,

	#[string = "Link number out of range"]
	Lnrng,

	#[string = "Protocol driver not attached"]
	Unatch,

	#[string = "No CSI structure available"]
	Nocsi,

	#[string = "Level 2 halted"]
	L2hlt,

	#[string = "Invalid exchange"]
	Bade,

	#[string = "Invalid request descriptor"]
	Badr,

	#[string = "Exchange full"]
	Xfull,

	#[string = "No anode"]
	Noano,

	#[string = "Invalid request code"]
	Badrqc,

	#[string = "Invalid slot"]
	Badslt,

	#[string = "Bad font file format"]
	BFont   = 59,

	#[string = "Device not a stream"]
	NoStr,

	#[string = "No data available"]
	NoData,

	#[string = "Timer expired"]
	Time,

	#[string = "Out of streams resources"]
	NoSr,

	#[string = "Machine is not on the network"]
	NoNet,

	#[string = "Package not installed"]
	NoPkg,

	#[string = "Object is remote"]
	Remote,

	#[string = "Link has been severed"]
	NoLink,

	#[string = "Advertise error"]
	Adv,

	#[string = "Srmount error"]
	Srmnt,

	#[string = "Communication error on send"]
	Comm,

	#[string = "Protocol error"]
	Proto,

	#[string = "Multihop attempted"]
	Multihop,

	#[string = "RFS specific error"]
	Dotdot,

	#[string = "Bad message"]
	BadMsg,

	#[string = "Value too large for defined data type"]
	Overflow,

	#[string = "Name not unique on network"]
	NotUniq,

	#[string = "File descriptor in bad state"]
	BadFd,

	#[string = "Remote address changed"]
	RemChg,

	#[string = "Can not access a needed shared library"]
	LibAcc,

	#[string = "Accessing a corrupted shared library"]
	LibBad,

	#[string = ".lib section in a.out corrupted"]
	LibScn,

	#[string = "Attempting to link in too many shared libraries"]
	LibMax,

	#[string = "Cannot exec a shared library directly"]
	LibExec,

	#[string = "Invalid or incomplete multibyte or wide character"]
	IlSeq,

	#[string = "Interrupted system call should be restarted"]
	Restart,

	#[string = "Streams pipe error"]
	StrPipe,

	#[string = "Too many users"]
	Users,

	#[string = "Socket operation on non-socket"]
	NotSock,

	#[string = "Destination address required"]
	DestAddrReq,

	#[string = "Message too long"]
	MsgSize,

	#[string = "Protocol wrong type for socket"]
	Prototype,

	#[string = "Protocol not available"]
	NoProtoOpt,

	#[string = "Protocol not supported"]
	ProtoNoSupport,

	#[string = "Socket type not supported"]
	SocktNoSupport,

	#[string = "Operation not supported"]
	OpNotSupp,

	#[string = "Protocol family not supported"]
	PfNoSupport,

	#[string = "Address family not supported by protocol"]
	AfNoSupport,

	#[string = "Address already in use"]
	AddrInUse,

	#[string = "Cannot assign requested address"]
	AddrNotAvail,

	#[string = "Network is down"]
	NetDown,

	#[string = "Network is unreachable"]
	NetUnreach,

	#[string = "Network dropped connection on reset"]
	NetReset,

	#[string = "Software caused connection abort"]
	ConnAborted,

	#[string = "Connection reset by peer"]
	ConnReset,

	#[string = "No buffer space available"]
	NoBufs,

	#[string = "Transport endpoint is already connected"]
	IsConn,

	#[string = "Transport endpoint is not connected"]
	NotConn,

	#[string = "Cannot send after transport endpoint shutdown"]
	Shutdown,

	#[string = "Too many references: cannot splice"]
	TooManyRefs,

	#[string = "Connection timed out"]
	TimedOut,

	#[string = "Connection refused"]
	ConnRefused,

	#[string = "Host is down"]
	HostDown,

	#[string = "No route to host"]
	HostUnreach,

	#[string = "Operation already in progress"]
	Already,

	#[string = "Operation now in progress"]
	InProgress,

	#[string = "Stale file handle"]
	Stale,

	#[string = "Structure needs cleaning"]
	UClean,

	#[string = "Not a XENIX named type file"]
	NotNam,

	#[string = "No XENIX semaphores available"]
	NAvail,

	#[string = "Is a named type file"]
	IsNam,

	#[string = "Remote I/O error"]
	RemoteIo,

	#[string = "Disk quota exceeded"]
	DQuot,

	#[string = "No medium found"]
	NoMedium,

	#[string = "Wrong medium type"]
	MediumType,

	#[string = "Operation canceled"]
	Canceled,

	#[string = "Required key not available"]
	NoKey,

	#[string = "Key has expired"]
	KeyExpired,

	#[string = "Key has been revoked"]
	KeyRevoked,

	#[string = "Key was rejected by service"]
	KeyRejected,

	#[string = "Owner died"]
	OwnerDead,

	#[string = "State not recoverable"]
	NotRecoverable,

	#[string = "Operation not possible due to RF-kill"]
	RfKill,

	#[string = "Memory page has hardware error"]
	HwPoison
}

impl From<OsError> for i16 {
	fn from(value: OsError) -> Self {
		value as Self
	}
}

impl From<OsError> for i32 {
	fn from(value: OsError) -> Self {
		value as Self
	}
}

impl From<i32> for OsError {
	fn from(value: i32) -> Self {
		Self::from_i32(value).unwrap_or(Self::Unknown)
	}
}

#[allow(non_upper_case_globals)]
impl OsError {
	pub const DeadLk: Self = Self::Deadlock;
	pub const WouldBlock: Self = Self::Again;
}

extern "C" {
	#[link_name = "__errno_location"]
	fn errno_location() -> MutPtr<i32>;
}

#[must_use]
#[allow(clippy::multiple_unsafe_ops_per_block)]
pub fn errno() -> i32 {
	/* Safety: this is always valid */
	unsafe { ptr!(*errno_location()) }
}

#[allow(clippy::multiple_unsafe_ops_per_block)]
pub fn set_errno(code: i32) {
	/* Safety: this is always valid */
	unsafe { ptr!(*errno_location()) = code };
}

#[allow(clippy::unwrap_used, clippy::missing_panics_doc)]
pub fn result_from_libc(result: isize) -> OsResult<isize> {
	if result >= 0 {
		return Ok(result);
	}

	let code = errno();

	Err(OsError::from(code))
}

pub fn result_from_int(result: isize) -> OsResult<isize> {
	if result >= 0 {
		return Ok(result);
	}

	#[allow(clippy::cast_possible_truncation, clippy::arithmetic_side_effects)]
	Err(OsError::from(-result as i32))
}

#[allow(clippy::cast_sign_loss)]
pub fn result_from_ptr(result: isize) -> OsResult<usize> {
	let err = -4096isize as usize;

	if result as usize <= err {
		return Ok(result as usize);
	}

	#[allow(clippy::cast_possible_truncation, clippy::arithmetic_side_effects)]
	Err(OsError::from(-result as i32))
}
