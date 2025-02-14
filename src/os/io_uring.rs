use super::error::OsError;
use super::signal::{SignalMask, SIGSET_SIZE};
use super::time::TimeSpec;
use super::*;

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum SetupFlag {
		IoPoll                  = 1 << 0,
		SubmissionQueuePolling  = 1 << 1,
		SubmissionQueueAffinity = 1 << 2,
		CompletionRingSize      = 1 << 3,
		Clamp                   = 1 << 4,
		AttachWq                = 1 << 5,
		RingDisabled            = 1 << 6,
		SubmitAll               = 1 << 7,
		CoopTaskrun             = 1 << 8,
		TaskrunFlag             = 1 << 9,
		SubmissionEntryWide     = 1 << 10,
		CompletionEntryWide     = 1 << 11,
		SingleIssuer            = 1 << 12,
		DeferTaskrun            = 1 << 13,
		NoMmap                  = 1 << 14,
		RegisteredFdOnly        = 1 << 15,
		NoSubmissionArray       = 1 << 16
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum Feature {
		SingleMmap     = 1 << 0,
		NoDrop         = 1 << 1,
		SubmitStable   = 1 << 2,
		RwCurPos       = 1 << 3,
		CurPersonality = 1 << 4,
		FastPoll       = 1 << 5,
		Poll32Bits     = 1 << 6,
		SqPollNonFixed = 1 << 7,
		ExtArg         = 1 << 8,
		NativeWorkers  = 1 << 9,
		RsrcTags       = 1 << 10,
		CqeSkip        = 1 << 11,
		LinkedFile     = 1 << 12,
		RegRegRing     = 1 << 13
	}
}

define_struct! {
	pub struct Parameters {
		pub sq_entries: u32,
		pub cq_entries: u32,
		pub flags: u32,
		pub sq_thread_cpu: u32,
		pub sq_thread_idle: u32,
		pub features: u32,
		pub wq_fd: u32,
		pub resv: [u32; 3],
		pub sq_off: SubmissionRingOffsets,
		pub cq_off: CompletionRingOffsets
	}
}

impl Parameters {
	#[must_use]
	pub fn flags(&self) -> BitFlags<SetupFlag> {
		BitFlags::from_bits_truncate(self.flags)
	}

	pub fn set_flags(&mut self, flags: BitFlags<SetupFlag>) {
		self.flags = flags.bits();
	}

	#[must_use]
	pub fn features(&self) -> BitFlags<Feature> {
		BitFlags::from_bits_truncate(self.features)
	}
}

define_enum! {
	#[bitflags]
	#[repr(u8)]
	pub enum SubmissionEntryFlag {
		FixedFile      = 1 << 0,
		IoDrain        = 1 << 1,
		IoLink         = 1 << 2,
		IoHardLink     = 1 << 3,
		Async          = 1 << 4,
		BufferSelect   = 1 << 5,
		CqeSkipSuccess = 1 << 6
	}
}

define_enum! {
	#[repr(u8)]
	pub enum OpCode {
		NoOp,
		ReadVector,
		WriteVector,
		FileSync,
		ReadFixed,
		WriteFixed,
		PollAdd,
		PollRemove,
		SyncFileRange,
		SendMsg,
		RecvMsg,
		Timeout,
		TimeoutRemove,
		Accept,
		AsyncCancel,
		LinkTimeout,
		Connect,
		FileAllocate,
		OpenAt,
		Close,
		FilesUpdate,
		Statx,
		Read,
		Write,
		FileAdvise,
		MemoryAdvise,
		Send,
		Recv,
		OpenAt2,
		EPollCtl,
		Splice,
		ProvideBuffers,
		RemoveBuffers,
		Tee,
		Shutdown,
		RenameAt,
		UnlinkAt,
		MkdirAt,
		SymlinkAt,
		LinkAt,
		MsgRing,
		FileSetXAttr,
		SetXAttr,
		FileGetXAttr,
		GetXAttr,
		Socket,
		UringCmd,
		SendZeroCopy,
		SendMsgZeroCopy,
		ReadMultishot,
		WaitId,
		FutexWait,
		FutexWake,
		FutexWaitV,
		FixedFdInstall,
		FileTruncate,
		Last
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum FileSyncFlags {
		DataSync = 1 << 0
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum TimeoutFlags {
		Abs               = 1 << 0,
		Update            = 1 << 1,
		BootTime          = 1 << 2,
		RealTime          = 1 << 3,
		LinkTimeoutUpdate = 1 << 4,
		ExpireIsSuccess   = 1 << 5,
		Multishot         = 1 << 6
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum SpliceFlag {
		FdInFixed = 1 << 31
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum PollAddFlag {
		Multi          = 1 << 0,
		UpdateEvents   = 1 << 1,
		UpdateUserData = 1 << 2,
		AddLevel       = 1 << 3
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum AsyncCancelFlag {
		All     = 1 << 0,
		Fd      = 1 << 1,
		Any     = 1 << 2,
		FdFixed = 1 << 3
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum RecvSendFlag {
		PollFirst               = 1 << 0,
		RecvMultishot           = 1 << 1,
		FixedBuf                = 1 << 2,
		SendZeroCopyReportUsage = 1 << 3
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum NotifFlag {
		SendCopied = 1 << 31
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum AcceptFlag {
		Multishot = 1 << 0
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum MsgRingOp {
		MsgData,
		MsgSendFd
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum MsgRingFlag {
		CqeSkip   = 1 << 0,
		FlagsPass = 1 << 1
	}
}

define_struct! {
	pub struct CmdOp {
		pub op: u32,
		pub pad: [u32; 1]
	}
}

define_union! {
	pub union Wide {
		pub off: u64,
		pub addr: u64,
		pub cmd_op: CmdOp,
		pub cmd: [u8; 0]
	}
}

define_struct! {
	pub struct AddrLen {
		pub len: u16,
		pub pad: [u16; 1]
	}
}

define_union! {
	pub union File {
		pub splice_fd_in: i32,
		pub file_index: u32,
		pub addr_len: AddrLen
	}
}

define_struct! {
	pub struct SubmissionEntry {
		pub op: OpCode,
		pub flags: u8,
		pub ioprio: u16,
		pub fd: i32,
		pub off: Wide,
		pub addr: Wide,
		pub len: u32,
		pub rw_flags: u32,
		pub user_data: u64,
		pub buf: u16,
		pub personality: u16,
		pub file: File,
		pub addr3: Wide,
		pub pad: [u64; 1]
	}
}

define_struct! {
	pub struct CompletionEntry {
		pub user_data: u64,
		pub result: i32,
		pub flags: u32
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum CompletionEntryFlag {
		Buffer         = 1 << 0,
		More           = 1 << 1,
		SocketReadable = 1 << 2,
		Notification   = 1 << 3
	}
}

define_enum! {
	#[repr(usize)]
	pub enum MmapOffsets {
		SubmissionRing     = 0x0,
		CompletionRing     = 0x0800_0000,
		SubmissionEntries  = 0x1000_0000,
		ProvideBuffersRing = 0x8000_0000
	}
}

define_struct! {
	pub struct SubmissionRingOffsets {
		pub head: u32,
		pub tail: u32,
		pub ring_mask: u32,
		pub ring_entries: u32,
		pub flags: u32,
		pub dropped: u32,
		pub array: u32,
		pub reserved: [u32; 1],
		pub user_addr: u64
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum SubmissionRingFlag {
		SqNeedWakeup = 1 << 0,
		CqOverflow   = 1 << 1,
		TaskRun      = 1 << 2
	}
}

define_struct! {
	pub struct CompletionRingOffsets {
		pub head: u32,
		pub tail: u32,
		pub ring_mask: u32,
		pub ring_entries: u32,
		pub overflow: u32,
		pub cqes: u32,
		pub flags: u32,
		pub resv: [u32; 1],
		pub user_addr: u64
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum CompletionRingFlag {
		EventFdDisabled = 1 << 0
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum EnterFlag {
		GetEvents      = 1 << 0,
		SqWakeup       = 1 << 1,
		SqWait         = 1 << 2,
		ExtArg         = 1 << 3,
		RegisteredRing = 1 << 4
	}
}

define_enum! {
	#[repr(u32)]
	pub enum RegisterOp {
		RegisterBuffers,
		UnregisterBuffers,
		RegisterFiles,
		UnregisterFiles,
		RegisterEventFd,
		UnregisterEventFd,
		RegisterFilesUpdate,
		RegisterEventFdAsync,
		RegisterProbe,
		RegisterPersonality,
		UnregisterPersonality,
		RegisterRestrictions,
		RegisterEnableRings,

		RegisterFiles2,
		RegisterFilesUpdate2,
		RegisterBuffers2,
		RegisterBuffersUpdate,

		RegisterIoWqAff,
		UnregisterIoWqAff,

		RegisterIoWqMaxWorkers,

		RegisterRingFds,
		UnregisterRingFds,

		RegisterPBufRing,
		UnregisterPBufRing,

		SyncCancel,

		RegisterFileAllocRange,

		Last,

		RegisterUseRegisteredRing = 1 << 31
	}
}

define_enum! {
	#[repr(u32)]
	pub enum WqCategory {
		Bound,
		Unbound
	}
}

define_struct! {
	#[deprecated]
	pub struct FilesUpdate {
		pub offset: u32,
		pub resv: [u32; 1],
		pub fds: u64
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum RsrcFlag {
		RegisterSparse = 1 << 0
	}
}

define_struct! {
	pub struct RsrcRegister {
		pub count: u32,
		pub flags: u32,
		pub resv: [u64; 1],
		pub data: u64,
		pub tags: u64
	}
}

define_struct! {
	pub struct RsrcUpdate {
		pub offset: u32,
		pub resv: [u32; 1],
		pub data: u64
	}
}

define_struct! {
	pub struct RsrcUpdate2 {
		pub offset: u32,
		pub resv: [u32; 1],
		pub data: u64,
		pub tags: u64,
		pub count: u32,
		pub resv2: [u32; 1]
	}
}

pub const REGISTER_FILES_SKIP: i32 = -2;

define_enum! {
	#[bitflags]
	#[repr(u16)]
	pub enum ProbeOpFlags {
		Supported = 1 << 0
	}
}

define_struct! {
	pub struct ProbeOp {
		pub op: u8,
		pub resv: [u8; 1],
		pub flags: u16,
		pub resv2: [u32; 1]
	}
}

impl ProbeOp {
	#[must_use]
	pub fn flags(&self) -> BitFlags<ProbeOpFlags> {
		BitFlags::from_bits_truncate(self.flags)
	}
}

define_struct! {
	pub struct ProbeReg {
		pub last_op: u8,
		pub length: u8,
		pub resv: [u16; 1],
		pub resv2: [u32; 3],
	}
}

define_struct! {
	pub struct ProbeDef<T: ?Sized> {
		pub probe: ProbeReg,
		pub ops: T
	}
}

pub const PROBE_OPS_COUNT: usize = 256;

pub type ProbeUnsized = ProbeDef<[ProbeOp]>;
pub type Probe = ProbeDef<[ProbeOp; PROBE_OPS_COUNT]>;

define_enum! {
	pub enum RestrictionOpCode {
		RegisterOp,
		SqeOp,
		SqeFlagsAllowed,
		SqeFlagsRequired
	}
}

define_struct! {
	pub struct Restriction {
		pub opcode: u16,
		pub union: u8,
		pub resv: [u8; 1],
		pub resv2: [u32; 3]
	}
}

define_struct! {
	pub struct Buf {
		pub addr: u64,
		pub len: u32,
		pub bid: u16,
		pub resv: [u16; 1]
	}
}

define_struct! {
	pub struct BufRing {
		pub resv: [u16; 7],
		pub tail: u16
	}
}

define_enum! {
	#[bitflags]
	#[repr(u32)]
	pub enum PBufRingFlag {
		Mmap = 1 << 0
	}
}

define_struct! {
	pub struct BufReg {
		pub ring_addr: u64,
		pub ring_entries: u32,
		pub bgid: u16,
		pub flags: u16,
		pub resv: [u64; 3]
	}
}

define_struct! {
	pub struct GetEventsArg {
		pub sig_mask: u64,
		pub sig_mask_size: u32,
		pub pad: [u32; 0],
		pub ts: u64
	}
}

define_struct! {
	pub struct SyncCancelReg {
		pub addr: u64,
		pub fd: i32,
		pub flags: u32,
		pub timeout: TimeSpec,
		pub pad: [u64; 4]
	}
}

define_struct! {
	pub struct FileIndexRange {
		pub off: u32,
		pub len: u32,
		pub resv: [u64; 1]
	}
}

define_struct! {
	pub struct RecvMsgOut {
		pub name_len: u32,
		pub control_len: u32,
		pub payload_len: u32,
		pub flags: u32
	}
}

define_enum! {
	pub enum SocketCmd {
		SIOCINQ,
		SIOCOUTQ
	}
}

/// # Safety
/// the entries to be submitted indicated by `submit` and the sqring's `head`
/// and `tail` pointers must be valid
pub unsafe fn io_uring_enter(
	fd: BorrowedFd<'_>, submit: u32, min_complete: u32, flags: BitFlags<EnterFlag>,
	sigset: SignalMask<'_>
) -> OsResult<i32> {
	/* Safety: guaranteed by caller */
	unsafe { io_uring_enter2(fd, submit, min_complete, flags, sigset.into()) }
}

/// # Safety
/// See [`io_uring_enter`]
#[syscall_define(IoUringEnter)]
pub unsafe fn io_uring_enter2(
	fd: BorrowedFd<'_>, submit: u32, min_complete: u32, flags: BitFlags<EnterFlag>,
	#[array] arg: RawBuf<'_>
) -> OsResult<i32>;

/// # Safety
/// See [`io_uring_enter`]
///
/// # Panics
/// if `timeout` does not fit in a i64
pub unsafe fn io_uring_enter_timeout(
	fd: BorrowedFd<'_>, submit: u32, min_complete: u32, mut flags: BitFlags<EnterFlag>,
	timeout: u64
) -> OsResult<i32> {
	let mut args = GetEventsArg::default();

	#[allow(clippy::expect_used)]
	let ts = TimeSpec {
		/* io_uring does not enforce nanos < 1e9 */
		nanos: timeout.try_into().expect("Valid timeout"),
		sec: 0
	};

	#[allow(clippy::cast_possible_truncation)]
	(args.sig_mask_size = SIGSET_SIZE as u32);
	args.ts = ptr!(&ts).addr() as u64;
	flags |= EnterFlag::ExtArg;

	/* Safety: guaranteed by caller */
	unsafe { io_uring_enter2(fd, submit, min_complete, flags, RawBuf::from(&[args])) }
}

#[syscall_define(IoUringSetup)]
pub fn io_uring_setup(entries: u32, params: &mut Parameters) -> OsResult<OwnedFd>;

/// # Safety
/// `arg` and `arg_count` must be valid for the respective RegisterOp
#[syscall_define(IoUringRegister)]
pub unsafe fn io_uring_register(
	fd: BorrowedFd<'_>, opcode: RegisterOp, arg: MutPtr<()>, arg_count: u32
) -> OsResult<i32>;

#[must_use]
pub fn io_uring_opcode_supported(ops: &[ProbeOp], op: OpCode) -> bool {
	if op as usize >= ops.len() {
		false
	} else {
		ops[op as usize].flags().intersects(ProbeOpFlags::Supported)
	}
}

pub fn io_uring_register_sync_cancel(
	fd: BorrowedFd<'_>, cancel: &mut SyncCancelReg
) -> OsResult<()> {
	/* Safety: args are valid */
	unsafe { io_uring_register(fd, RegisterOp::SyncCancel, ptr!(cancel).cast(), 1)? };

	Ok(())
}

define_struct! {
	pub struct IoRingFeatures {
		pub min_ver: u32,
		pub features: BitFlags<Feature>,
		pub setup_flags: BitFlags<SetupFlag>,
		pub ops: [bool; OpCode::Last as usize],
		pub register: [bool; RegisterOp::Last as usize + 1]
	}
}

impl IoRingFeatures {
	#[must_use]
	pub fn version(&self) -> String {
		format!("{}.{}", self.min_ver / 100, self.min_ver % 100)
	}

	#[must_use]
	pub fn feature_supported(&self, feature: Feature) -> bool {
		self.features.intersects(feature)
	}

	#[must_use]
	pub fn setup_flag_supported(&self, flag: SetupFlag) -> bool {
		self.setup_flags.intersects(flag)
	}

	#[must_use]
	pub const fn opcode_supported(&self, op: OpCode) -> bool {
		self.ops[op as usize]
	}

	#[must_use]
	pub fn register_op_supported(&self, op: RegisterOp) -> bool {
		let index = if op == RegisterOp::RegisterUseRegisteredRing {
			RegisterOp::Last as usize
		} else {
			op as usize
		};

		self.register[index]
	}
}

const FEATURE_MAP: &[(Feature, u32)] = &[
	(Feature::SingleMmap, 504),
	(Feature::NoDrop, 505),
	(Feature::SubmitStable, 505),
	(Feature::RwCurPos, 506),
	(Feature::CurPersonality, 506),
	(Feature::FastPoll, 507),
	(Feature::Poll32Bits, 509),
	(Feature::SqPollNonFixed, 511),
	(Feature::ExtArg, 511),
	(Feature::NativeWorkers, 512),
	(Feature::RsrcTags, 513),
	(Feature::CqeSkip, 517),
	(Feature::LinkedFile, 517),
	(Feature::RegRegRing, 603)
];

const OP_MAP: &[(OpCode, u32)] = &[
	(OpCode::NoOp, 501),
	(OpCode::ReadVector, 501),
	(OpCode::WriteVector, 501),
	(OpCode::ReadFixed, 501),
	(OpCode::WriteFixed, 501),
	(OpCode::FileSync, 501),
	(OpCode::PollAdd, 501),
	(OpCode::PollRemove, 501),
	(OpCode::SyncFileRange, 502),
	(OpCode::SendMsg, 503),
	(OpCode::RecvMsg, 503),
	(OpCode::Timeout, 504),
	(OpCode::TimeoutRemove, 505),
	(OpCode::Accept, 505),
	(OpCode::AsyncCancel, 505),
	(OpCode::LinkTimeout, 505),
	(OpCode::Connect, 505),
	(OpCode::EPollCtl, 506),
	(OpCode::Send, 506),
	(OpCode::Recv, 506),
	(OpCode::FileAllocate, 506),
	(OpCode::FileAdvise, 506),
	(OpCode::MemoryAdvise, 506),
	(OpCode::OpenAt, 506),
	(OpCode::OpenAt2, 506),
	(OpCode::Close, 506),
	(OpCode::Statx, 506),
	(OpCode::Read, 506),
	(OpCode::Write, 506),
	(OpCode::FilesUpdate, 506),
	(OpCode::Splice, 507),
	(OpCode::ProvideBuffers, 507),
	(OpCode::RemoveBuffers, 507),
	(OpCode::Tee, 508),
	(OpCode::Shutdown, 511),
	(OpCode::RenameAt, 511),
	(OpCode::UnlinkAt, 511),
	(OpCode::MkdirAt, 515),
	(OpCode::SymlinkAt, 515),
	(OpCode::LinkAt, 515),
	(OpCode::MsgRing, 518),
	(OpCode::FileSetXAttr, 519),
	(OpCode::SetXAttr, 519),
	(OpCode::FileGetXAttr, 519),
	(OpCode::GetXAttr, 519),
	(OpCode::Socket, 519),
	(OpCode::UringCmd, 519),
	(OpCode::SendZeroCopy, 600),
	(OpCode::SendMsgZeroCopy, 601),
	(OpCode::ReadMultishot, 607),
	(OpCode::WaitId, 607),
	(OpCode::FutexWait, 607),
	(OpCode::FutexWake, 607),
	(OpCode::FutexWaitV, 607),
	(OpCode::FixedFdInstall, 608),
	(OpCode::FileTruncate, 609)
];

const SETUP_FLAG_MAP: &[(SetupFlag, u32)] = &[
	(SetupFlag::IoPoll, 501),
	(SetupFlag::SubmissionQueuePolling, 501),
	(SetupFlag::SubmissionQueueAffinity, 501),
	(SetupFlag::CompletionRingSize, 505),
	(SetupFlag::Clamp, 506),
	(SetupFlag::AttachWq, 506),
	(SetupFlag::RingDisabled, 510),
	(SetupFlag::SubmitAll, 518),
	(SetupFlag::CoopTaskrun, 519),
	(SetupFlag::TaskrunFlag, 519),
	(SetupFlag::SubmissionEntryWide, 519),
	(SetupFlag::CompletionEntryWide, 519),
	(SetupFlag::SingleIssuer, 600),
	(SetupFlag::DeferTaskrun, 601),
	// TODO: these need more work
	(SetupFlag::NoMmap, 605),
	(SetupFlag::RegisteredFdOnly, 605),
	(SetupFlag::NoSubmissionArray, 606)
];

const REGISTER_MAP: &[(RegisterOp, u32)] = &[
	(RegisterOp::RegisterBuffers, 501),
	(RegisterOp::UnregisterBuffers, 501),
	(RegisterOp::RegisterFiles, 501),
	(RegisterOp::UnregisterFiles, 501),
	(RegisterOp::RegisterEventFd, 502),
	(RegisterOp::UnregisterEventFd, 502),
	(RegisterOp::RegisterFilesUpdate, 505),
	(RegisterOp::RegisterEventFdAsync, 506),
	(RegisterOp::RegisterProbe, 506),
	(RegisterOp::RegisterPersonality, 506),
	(RegisterOp::UnregisterPersonality, 506),
	(RegisterOp::RegisterRestrictions, 510),
	(RegisterOp::RegisterEnableRings, 510),
	(RegisterOp::RegisterFiles2, 513),
	(RegisterOp::RegisterFilesUpdate2, 513),
	(RegisterOp::RegisterBuffers2, 513),
	(RegisterOp::RegisterBuffersUpdate, 513),
	(RegisterOp::RegisterIoWqAff, 514),
	(RegisterOp::UnregisterIoWqAff, 514),
	(RegisterOp::RegisterIoWqMaxWorkers, 515),
	(RegisterOp::RegisterRingFds, 518),
	(RegisterOp::UnregisterRingFds, 518),
	(RegisterOp::RegisterPBufRing, 519),
	(RegisterOp::UnregisterPBufRing, 519),
	(RegisterOp::SyncCancel, 600),
	(RegisterOp::RegisterFileAllocRange, 600),
	(RegisterOp::RegisterUseRegisteredRing, 603)
];

#[allow(clippy::missing_panics_doc)]
pub fn io_uring_detect_features() -> OsResult<Option<IoRingFeatures>> {
	let mut params = Parameters { sq_entries: 1, ..Default::default() };

	let fd = match io_uring_setup(params.sq_entries, &mut params) {
		Ok(fd) => fd,
		Err(err) => match err {
			OsError::NoSys => return Ok(None),
			_ => return Err(err)
		}
	};

	let mut probe = Probe::default();

	/* Safety: valid probe object */
	let probe_result = unsafe {
		io_uring_register(
			fd.as_fd(),
			RegisterOp::RegisterProbe,
			ptr!(&mut probe).cast(),
			#[allow(clippy::cast_possible_truncation)]
			(PROBE_OPS_COUNT as u32)
		)
	};

	let mut features = IoRingFeatures {
		min_ver: 501,
		features: params.features(),
		setup_flags: BitFlags::default(),
		ops: [false; OpCode::Last as usize],
		register: [false; RegisterOp::Last as usize + 1]
	};

	for (feature, version) in FEATURE_MAP.iter().rev() {
		if params.features().intersects(*feature) {
			features.min_ver = *version;

			break;
		}
	}

	if let Err(err) = probe_result {
		match err {
			OsError::Inval => (),
			_ => return Err(err)
		}

		for (op, version) in OP_MAP {
			if features.min_ver < *version {
				break;
			}

			features.ops[*op as usize] = true;
		}
	} else {
		features.min_ver = features.min_ver.max(506);

		#[allow(clippy::arithmetic_side_effects)]
		let ops = &probe.ops[..probe.probe.last_op as usize + 1];

		for (op, version) in OP_MAP {
			let supported = io_uring_opcode_supported(ops, *op);

			features.ops[*op as usize] = supported;

			if supported {
				features.min_ver = features.min_ver.max(*version);
			}
		}
	}

	for (flag, version) in SETUP_FLAG_MAP.iter().rev() {
		if features.min_ver >= *version {
			features.setup_flags |= *flag;
		} else {
			let mut params = Parameters {
				sq_entries: 1,
				flags: *flag as u32,
				..Default::default()
			};

			if io_uring_setup(params.sq_entries, &mut params).is_ok() {
				features.min_ver = *version;
			}
		}
	}

	for (op, version) in REGISTER_MAP {
		if features.min_ver < *version {
			break;
		}

		#[allow(clippy::arithmetic_side_effects)]
		let index = if *op == RegisterOp::RegisterUseRegisteredRing {
			RegisterOp::Last as usize
		} else {
			*op as usize
		};

		features.register[index] = true;
	}

	Ok(Some(features))
}
