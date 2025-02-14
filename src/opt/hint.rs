use std::hint;

#[inline(always)]
#[cold]
const fn cold() {}

#[inline(always)]
#[must_use]
pub const fn expect(cond: bool, expect: bool) -> bool {
	if cond != expect {
		cold();
	}

	cond
}

#[inline(always)]
#[must_use]
pub const fn likely(cond: bool) -> bool {
	expect(cond, true)
}

#[inline(always)]
#[must_use]
pub const fn unlikely(cond: bool) -> bool {
	expect(cond, false)
}

/// # Safety
/// See [`std::hint::unreachable_unchecked`]
#[inline(always)]
pub const unsafe fn unreachable_unchecked() -> ! {
	/* Safety: guaranteed by caller */
	unsafe { hint::unreachable_unchecked() };
}

/// # Safety
/// See [`std::intrinsics::assume`]
#[inline(always)]
pub const unsafe fn assume(condition: bool) {
	if !condition {
		/* Safety: contract upheld by caller */
		unsafe { unreachable_unchecked() };
	}
}
