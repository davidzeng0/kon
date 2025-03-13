use super::*;

/// The error returned from a call to `recv`
#[errors]
pub enum RecvError {
	/// The channel is currently empty. Note that async `recv`s can return this
	/// variant if the current task gets interrupted
	#[display("Channel empty")]
	#[kind = ErrorKind::WouldBlock]
	Empty,

	/// The channel is closed
	#[display("Channel closed")]
	Closed
}

/// The error returned from a call to `send`
#[errors(?Debug + ?Display)]
pub enum SendError<T> {
	/// The channel is currently full. Note that async `send`s can return this
	/// variant if the current task gets interrupted
	#[fmt("Channel full")]
	#[kind = ErrorKind::WouldBlock]
	Full(T),

	/// The channel is closed
	#[fmt("Channel closed")]
	Closed(T)
}
