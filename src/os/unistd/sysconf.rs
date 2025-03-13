use super::*;

define_enum! {
	#[repr(i32)]
	pub enum SystemConfiguration {
		ArgMax,
		ChildMax,
		ClkTck,
		NgroupsMax,
		OpenMax,
		StreamMax,
		TznameMax,
		JobControl,
		SavedIds,
		RealtimeSignals,
		PriorityScheduling,
		Timers,
		AsynchronousIo,
		PrioritizedIo,
		SynchronizedIo,
		Fsync,
		MappedFiles,
		Memlock,
		MemlockRange,
		MemoryProtection,
		MessagePassing,
		Semaphores,
		SharedMemoryObjects,
		AioListioMax,
		AioMax,
		AioPrioDeltaMax,
		DelaytimerMax,
		MqOpenMax,
		MqPrioMax,
		Version,
		Pagesize,

		RtsigMax,
		SemNsemsMax,
		SemValueMax,
		SigqueueMax,
		TimerMax,

		BcBaseMax,
		BcDimMax,
		BcScaleMax,
		BcStringMax,
		CollWeightsMax,
		EquivClassMax,
		ExprNestMax,
		LineMax,
		ReDupMax,
		CharclassNameMax,

		TwoVersion,
		TwoCBind,
		TwoCDev,
		TwoFortDev,
		TwoFortRun,
		TwoSwDev,
		TwoLocaledef,

		Pii,
		PiiXti,
		PiiSocket,
		PiiInternet,
		PiiOsi,
		Poll,
		Select,
		UioMaxiov,
		PiiInternetStream,
		PiiInternetDgram,
		PiiOsiCots,
		PiiOsiClts,
		PiiOsiM,
		TIovMax,

		Threads,
		ThreadSafeFunctions,
		GetgrRSizeMax,
		GetpwRSizeMax,
		LoginNameMax,
		TtyNameMax,
		ThreadDestructorIterations,
		ThreadKeysMax,
		ThreadStackMin,
		ThreadThreadsMax,
		ThreadAttrStackaddr,
		ThreadAttrStacksize,
		ThreadPriorityScheduling,
		ThreadPrioInherit,
		ThreadPrioProtect,
		ThreadProcessShared,

		NprocessorsConf,
		NprocessorsOnln,
		PhysPages,
		AvphysPages,
		AtexitMax,
		PassMax,

		XopenVersion,
		XopenXcuVersion,
		XopenUnix,
		XopenCrypt,
		XopenEnhI18n,
		XopenShm,

		TwoCharTerm,
		TwoCVersion,
		TwoUpe,

		XopenXpg2,
		XopenXpg3,
		XopenXpg4,

		CharBit,
		CharMax,
		CharMin,
		IntMax,
		IntMin,
		LongBit,
		WordBit,
		MbLenMax,
		Nzero,
		SsizeMax,
		ScharMax,
		ScharMin,
		ShrtMax,
		ShrtMin,
		UcharMax,
		UintMax,
		UlongMax,
		UshrtMax,

		NlArgmax,
		NlLangmax,
		NlMsgmax,
		NlNmax,
		NlSetmax,
		NlTextmax,

		Xbs5Ilp32Off32,
		Xbs5Ilp32Offbig,
		Xbs5Lp64Off64,
		Xbs5LpbigOffbig,

		XopenLegacy,
		XopenRealtime,
		XopenRealtimeThreads,

		AdvisoryInfo,
		Barriers,
		Base,
		CLangSupport,
		CLangSupportR,
		ClockSelection,
		Cputime,
		ThreadCputime,
		DeviceIo,
		DeviceSpecific,
		DeviceSpecificR,
		FdMgmt,
		Fifo,
		Pipe,
		FileAttributes,
		FileLocking,
		FileSystem,
		MonotonicClock,
		MultiProcess,
		SingleProcess,
		Networking,
		ReaderWriterLocks,
		SpinLocks,
		Regexp,
		RegexVersion,
		Shell,
		Signals,
		Spawn,
		SporadicServer,
		ThreadSporadicServer,
		SystemDatabase,
		SystemDatabaseR,
		Timeouts,
		TypedMemoryObjects,
		UserGroups,
		UserGroupsR,
		TwoPbs,
		TwoPbsAccounting,
		TwoPbsLocate,
		TwoPbsMessage,
		TwoPbsTrack,
		SymloopMax,
		Streams,
		TwoPbsCheckpoint,

		V6Ilp32Off32,
		V6Ilp32Offbig,
		V6Lp64Off64,
		V6LpbigOffbig,

		HostNameMax,
		Trace,
		TraceEventFilter,
		TraceInherit,
		TraceLog,

		Level1IcacheSize,
		Level1IcacheAssoc,
		Level1IcacheLinesize,
		Level1DcacheSize,
		Level1DcacheAssoc,
		Level1DcacheLinesize,
		Level2CacheSize,
		Level2CacheAssoc,
		Level2CacheLinesize,
		Level3CacheSize,
		Level3CacheAssoc,
		Level3CacheLinesize,
		Level4CacheSize,
		Level4CacheAssoc,
		Level4CacheLinesize,

		Ipv6 = 235,
		RawSockets,
		V7Ilp32Off32,
		V7Ilp32Offbig,
		V7Lp64Off64,
		V7LpbigOffbig,
		SsReplMax,
		TraceEventNameMax,
		TraceNameMax,
		TraceSysMax,
		TraceUserEventMax,
		XopenStreams,
		ThreadRobustPrioInherit,
		ThreadRobustPrioProtect,
		Minsigstksz,
		Sigstksz
	}
}

#[allow(non_upper_case_globals)]
impl SystemConfiguration {
	pub const IovMax: Self = Self::UioMaxiov;
}

extern "C" {
	fn sysconf(name: i32) -> i64;
}

pub fn get_system_configuration(name: SystemConfiguration) -> OsResult<Option<u64>> {
	set_errno(0);

	/* Safety: this function is safe to call */
	let result = unsafe { sysconf(name as i32) };

	if result >= 0 {
		#[allow(clippy::cast_sign_loss)]
		return Ok(Some(result as u64));
	}

	let code = errno();

	if code == 0 {
		return Ok(None);
	}

	Err(OsError::from(code))
}
