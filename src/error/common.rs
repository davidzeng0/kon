use super::*;

pub const INVALID_UTF8: &SimpleMessage = &SimpleMessage {
	kind: ErrorKind::InvalidData,
	message: "Processed invalid UTF-8"
};

pub const NO_ADDRESSES: &SimpleMessage = &SimpleMessage {
	kind: ErrorKind::NoData,
	message: "Address list empty"
};

pub const INVALID_CSTR: &SimpleMessage = &SimpleMessage {
	kind: ErrorKind::InvalidInput,
	message: "Path string contained a null byte"
};

pub const CONNECT_TIMEOUT: &SimpleMessage = &SimpleMessage {
	kind: ErrorKind::TimedOut,
	message: "Connect timed out"
};
