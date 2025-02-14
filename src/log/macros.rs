#[macro_export]
macro_rules! log {
	($level: expr, target: $target: expr, $($arg: tt)+) => {
		if $crate::opt::hint::unlikely($crate::log::internal::log_enabled!($level)) {
			$crate::log::internal::log_target(
				$level,
				$crate::pointer::ptr!($target),
				format_args!($($arg)+)
			);
		}
	};

	($level: expr, $($arg: tt)+) => {
		$crate::log::internal::log!($level, $($arg)+)
	};
}

#[macro_export]
macro_rules! error {
	($($arg: tt)+) => {
		$crate::log!($crate::log::Level::Error, $($arg)+)
	}
}

#[macro_export]
macro_rules! warn {
	($($arg: tt)+) => {
		$crate::log!($crate::log::Level::Warn, $($arg)+)
	}
}

#[macro_export]
macro_rules! info {
	($($arg: tt)+) => {
		$crate::log!($crate::log::Level::Info, $($arg)+)
	}
}

#[macro_export]
macro_rules! debug {
	($($arg: tt)+) => {
		$crate::log!($crate::log::Level::Debug, $($arg)+)
	}
}

#[macro_export]
macro_rules! trace {
	($($arg: tt)+) => {
		$crate::log!($crate::log::Level::Trace, $($arg)+)
	}
}
