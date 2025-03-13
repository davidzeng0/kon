#[doc(inline)]
pub use xx_core_macros::*;
#[doc(inline)]
pub use xx_macros::macros::*;
#[doc(inline)]
pub use xx_proc_macros::*;

#[cfg(feature = "runtime")]
pub mod runtime;

#[cfg(feature = "runtime")]
#[doc(inline)]
pub use runtime::*;

#[macro_export]
macro_rules! macro_each {
	($macro:ident, $item:tt $($each:tt)*) => {
		$macro!($item);

		$crate::macros::macro_each!($macro $($each)*);
	};

	($macro:ident) => {}
}

pub use macro_each;

#[macro_export]
macro_rules! import_sysdeps {
	() => {
		#[cfg(any(target_arch = "aarch64", doc))]
		mod arm64;
		#[cfg(any(target_arch = "x86_64", doc))]
		mod x64;

		mod platform {
			#[cfg(target_arch = "aarch64")]
			#[allow(unused_imports)]
			pub use super::arm64::*;
			#[cfg(target_arch = "x86_64")]
			#[allow(unused_imports)]
			pub use super::x64::*;
		}

		#[allow(unused_imports)]
		use platform::*;
	};
}

pub use import_sysdeps;

#[macro_export]
macro_rules! require_unsafe {
	() => {{
		#[allow(clippy::missing_safety_doc)]
		const unsafe fn require_unsafe() {}

		require_unsafe();
	}};
}

pub use require_unsafe;

#[macro_export]
macro_rules! const_assert {
	($expr:expr $(, $($args:tt)*)?) => {
		const _: () = {
			assert!($expr $(, $($args)*)?)
		};
	};
}

pub use const_assert;
