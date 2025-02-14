use std::os::unix::thread::RawPthread;

use super::error::result_from_libc;
use super::*;

pub type SignalSet = u64;

define_enum! {
	#[repr(i32)]
	pub enum Signal {
		/// Hang up controlling terminal or process
		Hangup = 1,

		/// Interrupt from keyboard, ^C
		Interrupt,

		/// Quit from keyboard, ^\
		Quit,

		IllegalInstruction,

		/// Breakpoint for debugging
		Trap,

		/// Abnormal termination
		Abort,

		/// Bus error
		Bus,

		FloatingPointException,

		/// Forced process termination
		Kill,

		/// User-defined signal 1
		User1,

		/// Invalid memory reference
		SegmentationViolation,

		/// User-defined signal 2
		User2,

		/// Write to pipe with no readers
		Pipe,

		/// Real-timer clock
		Alarm,

		/// Process termination
		Termination,

		/// Stack fault (obsolete)
		StackFault,

		/// Child process terminated or stopped
		Child,

		/// Resume execution, if stopped
		Continue,

		/// Stop process execution, ^Z
		Stop,

		/// Keyboard stop
		TTYStop,

		/// Background read from control terminal
		TTIn,

		/// Background write to control terminal
		TTOut,

		/// Urgent condition on socket
		Urgent,

		/// CPU time limit exceeded
		ExceedCpu,

		/// File size limit exceeded
		ExceedFileSize,

		/// Virtual timer expired
		VirtualAlarm,

		/// Profiling timer expired
		Profile,

		/// Window size change
		WindowChange,

		/// I/O now possible
		Io,

		/// Power failure imminent
		Power,

		/// Bad system call
		Syscall
	}
}

#[allow(non_upper_case_globals)]
impl Signal {
	/// IOT instruction, abort
	pub const Iot: Self = Self::Abort;
	/// Equivalent to Io
	pub const Poll: Self = Self::Io;
	/// Equivalent to Syscall
	pub const Unused: Self = Self::Syscall;
}

pub const SIGRTMIN: u32 = 32;
pub const SIGRTMAX: u32 = 65;
pub const SIGSET_SIZE: usize = SIGRTMAX as usize / 8;

pub type SignalMask<'a> = Option<&'a [SignalSet]>;
pub type SignalMaskMut<'a> = Option<&'a mut [SignalSet]>;

impl<'a> From<SignalMask<'a>> for RawBuf<'a> {
	fn from(value: SignalMask<'a>) -> Self {
		let parts = value.into_raw_array();

		Self::from_parts(parts.0.cast(), parts.1)
	}
}

define_union! {
	pub union SigVal {
		pub int: i32,
		pub ptr: MutPtr<()>
	}
}

define_struct! {
	pub struct SigKill {
		pub pid: i32,
		pub uid: u32
	}
}

define_struct! {
	pub struct SigTimer {
		pub tid: i32,
		pub overrun: i32,
		pub sigval: SigVal,
		pub private: i32
	}
}

define_struct! {
	pub struct SigRt {
		pub pid: i32,
		pub uid: u32,
		pub sigval: SigVal
	}
}

define_struct! {
	pub struct SigChild {
		pub pid: i32,
		pub uidi: u32,
		pub status: i32,
		pub utime: i64,
		pub stime: i64
	}
}

define_struct! {
	pub struct SigFaultAddrBnd {
		pub pad: MutPtr<()>,
		pub lower: MutPtr<()>,
		pub upper: MutPtr<()>
	}
}

define_struct! {
	pub struct SigFaultAddrPkey {
		pub pad: MutPtr<()>,
		pub pkey: u32
	}
}

define_struct! {
	pub struct SigFaultPerf {
		pub data: u64,
		pub ty: u32,
		pub flags: u32
	}
}

define_union! {
	pub union SigFaultInfo {
		pub trapno: i32,
		pub addr_lsb: i16,
		pub bnd: SigFaultAddrBnd,
		pub pkey: SigFaultAddrPkey,
		pub perf: SigFaultPerf
	}
}

define_struct! {
	pub struct SigFault {
		pub addr: MutPtr<()>,
		pub info: SigFaultInfo
	}
}

define_struct! {
	pub struct SigPoll {
		pub band: i64,
		pub fd: i32
	}
}

define_struct! {
	pub struct SigSys {
		pub addr: MutPtr<()>,
		pub syscall: i32,
		pub arch: u32
	}
}

define_union! {
	pub union SigFields {
		pub kill: SigKill,
		pub timer: SigTimer,
		pub rt: SigRt,
		pub child: SigChild,
		pub fault: SigFault,
		pub poll: SigPoll,
		pub sys: SigSys
	}
}

define_struct! {
	pub struct SigInfo {
		pub signal: i32,
		pub errno: i32,
		pub code: i32,
		pub fields: SigFields,
		pub pad: [u8; 80]
	}
}

define_enum! {
	#[repr(usize)]
	pub enum SigHandlers {
		Default,
		Ignore
	}
}

define_enum! {
	#[repr(u32)]
	#[bitflags]
	pub enum SignalFlags {
		NoCldStop     = 1 << 0,
		NoCldWait     = 1 << 1,
		SigInfo       = 1 << 2,
		Unsupported   = 1 << 10,
		ExposeTagBits = 1 << 11,
		Restorer      = 1 << 26,
		OnStack       = 1 << 27,
		Restart       = 1 << 28,
		NoDefer       = 1 << 30,
		ResetHand     = 1 << 31
	}
}

define_union! {
	pub union SigHandler {
		pub basic: SigHandlers,
		pub handler: Option<unsafe extern "C" fn(i32)>,
		pub action: Option<unsafe extern "C" fn(i32, MutPtr<SigInfo>, MutPtr<()>)>
	}
}

define_struct! {
	pub struct SigAction {
		pub handler: SigHandler,
		pub flags: u32,
		pub restorer: Option<unsafe extern "C" fn() -> !>,
		pub mask: [SignalSet; 16]
	}
}

define_enum! {
	#[repr(i32)]
	pub enum SignalHow {
		Block   = 0,
		Unblock = 1,
		SetMask = 2
	}
}

extern "C" {
	fn sigaction(num: i32, action: Ptr<SigAction>, old: MutPtr<SigAction>) -> i32;

	fn pthread_kill(thread: RawPthread, sig: i32) -> i32;
	fn pthread_sigqueue(thread: RawPthread, sig: i32, sigval: SigVal) -> i32;
	fn pthread_sigmask(how: i32, set: Ptr<SignalSet>, old: MutPtr<SignalSet>) -> i32;
}

pub fn sig_action(
	sig: i32, action: Option<&SigAction>, old: Option<&mut SigAction>
) -> OsResult<()> {
	/* Safety: ptrs are valid */
	let result = unsafe { sigaction(sig.into_raw(), action.into_raw(), old.into_raw()) };

	result_from_libc(result as isize).map(|_| ())
}

pub fn pthread_signal(thread: RawPthread, sig: i32) -> OsResult<()> {
	/* Safety: this function is safe to call */
	result_from_libc(unsafe { pthread_kill(thread, sig) } as isize).map(|_| ())
}

pub fn pthread_queue_signal(thread: RawPthread, sig: i32, sigval: SigVal) -> OsResult<()> {
	/* Safety: this function is safe to call */
	result_from_libc(unsafe { pthread_sigqueue(thread, sig, sigval) } as isize).map(|_| ())
}

pub fn pthread_set_sigmask(
	how: SignalHow, set: SignalMask<'_>, old: SignalMaskMut<'_>
) -> OsResult<()> {
	/* Safety: this function is safe to call */
	let result =
		unsafe { pthread_sigmask(how as i32, set.into_raw_array().0, old.into_raw_array().0) };

	result_from_libc(result as isize).map(|_| ())
}
