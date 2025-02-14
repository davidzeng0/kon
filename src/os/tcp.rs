use super::*;

define_enum! {
	#[repr(u32)]
	pub enum TcpOption {
		/// Don't delay send to coalesce packets
		NoDelay             = 1,

		/// Set maximum segment size
		MaxSegment          = 2,

		/// Control sending of partial frames
		Cork                = 3,

		/// Start keeplives after this period
		KeepIdle            = 4,

		/// Interval between keepalives
		KeepInterval        = 5,

		/// Number of keepalives before death
		KeepCount           = 6,

		/// Number of SYN retransmits
		SynCount            = 7,

		/// Life time of orphaned FIN-WAIT-2 state
		Linger2             = 8,

		/// Wake up listener only when data arrive
		DeferAccept         = 9,

		/// Bound advertised window
		WindowClamp         = 10,

		/// Information about this connection.
		Info                = 11,

		/// Bock/reenable quick ACKs.
		Quickack            = 12,

		/// Congestion control algorithm.
		Congestion          = 13,

		/// TCP MD5 Signature (RFC2385)
		Md5Signature        = 14,

		/// TCP Cookie Transactions
		CookieTransactions  = 15,

		/// Use linear timeouts for thin streams
		ThinLinearTimeouts  = 16,

		/// Fast retrans. after 1 dupack
		ThinDupAck          = 17,

		/// How long for loss retry before timeout
		UserTimeout         = 18,

		/// TCP sock is under repair right now
		Repair              = 19,

		/// Set TCP queue to repair
		RepairQueue         = 20,

		/// Set sequence number of repaired queue.
		QueueSeq            = 21,

		/// Repair TCP connection options
		RepairOptions       = 22,

		/// Enable FastOpen on listeners
		Fastopen            = 23,

		/// TCP time stamp
		Timestamp           = 24,

		/// Limit number of unsent bytes in write queue.
		NotSentLowWatermark = 25,

		/// Get Congestion Control (optional) info.
		CcInfo              = 26,

		/// Record SYN headers for new connections.
		SaveSyn             = 27,

		/// Get SYN headers recorded for connection.
		SavedSyn            = 28,

		/// Get/set window parameters.
		RepairWindow        = 29,

		/// Attempt FastOpen with connect.
		FastopenConnect     = 30,

		/// Attach a ULP to a TCP connection.
		Ulp                 = 31,

		/// TCP MD5 Signature with extensions.
		Md5sigExt           = 32,

		/// Set the key for Fast Open (cookie).
		FastopenKey         = 33,

		/// Enable TFO without a TFO cookie.
		FastopenNoCookie    = 34,

		ZerocopyReceive     = 35,

		/// Notify bytes available to read as a cmsg on read.
		Inq                 = 36,

		/// Delay outgoing packets by XX usec.
		TxDelay             = 37
	}
}

#[allow(non_upper_case_globals)]
impl TcpOption {
	pub const CmInq: Self = Self::Inq;
}
