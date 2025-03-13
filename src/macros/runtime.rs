#[macro_export]
macro_rules! panic_nounwind {
	($($arg: tt)*) => {
		$crate::runtime::panic_nounwind(::std::format_args!($($arg)*))
	}
}

pub use panic_nounwind;

#[macro_export]
macro_rules! unreachable_unchecked {
	($($arg: tt)*) => {{
		#[cfg(debug_assertions)]
		$crate::macros::require_unsafe!();

		#[cfg(debug_assertions)]
		$crate::macros::panic_nounwind!(
			"Entered unreachable code: {}",
			::std::format_args!($($arg)*)
		);

		#[cfg(not(debug_assertions))]
		let _ = || {
			::std::format_args!($($arg)*);
		};

		#[cfg(not(debug_assertions))]
		$crate::opt::hint::unreachable_unchecked();
	}}
}

pub use unreachable_unchecked;

#[macro_export]
macro_rules! assert_unsafe_precondition {
	($condition:expr) => {
		$crate::macros::assert_unsafe_precondition!(
			$condition,
			::std::stringify!($condition)
		)
	};

	($condition:expr, $($arg: tt)*) => {{
		#[cfg(debug_assertions)]
		if !$condition {
			$crate::macros::require_unsafe!();

			$crate::macros::panic_nounwind!(
				"Unsafe precondition(s) violated: {}",
				::std::format_args!($($arg)*)
			);
		}

		#[cfg(not(debug_assertions))]
		$crate::opt::hint::assume($condition);
	}}
}

pub use assert_unsafe_precondition;
