use std::arch::asm;

use super::*;

define_context! {
	pub struct Context {
		x19: usize,
		x29: usize,
		stack: usize,
		link: usize
	}
}

global_asm!(include_str!("arm64.s"));

extern "C" {
	fn xx_core_fiber_arm64_start();
	fn xx_core_fiber_arm64_intercept();
	fn xx_core_fiber_arm64_switch(from: MutPtr<Context>, to: MutPtr<Context>);
}

impl Context {
	pub const fn program_counter(&self) -> Ptr<()> {
		Ptr::from_addr(self.link)
	}

	/// # Safety
	/// the fiber must not be running
	pub unsafe fn set_stack(&mut self, stack: Ptr<()>, len: usize) {
		#[allow(clippy::arithmetic_side_effects)]
		(self.stack = stack.addr() + len);
	}

	/// # Safety
	/// the fiber must not be running
	pub unsafe fn set_start(&mut self, start: Start) {
		let stack = MutPtr::<Start>::from_addr(self.stack);

		/* Safety: guaranteed by caller */
		unsafe { stack.sub(1).write(start) };

		self.link = xx_core_fiber_arm64_start as usize;
	}

	/// # Safety
	/// valid intercept
	pub unsafe fn set_intercept(&mut self, intercept: Intercept) {
		let stack = MutPtr::<Intercept>::from_addr(self.stack);

		/* Safety: guaranteed by caller */
		unsafe { stack.sub(1).write(intercept) };

		self.link = xx_core_fiber_arm64_intercept as usize;
	}
}

/// # Safety
/// `from` is the current running worker
/// `to` is valid
pub unsafe fn switch(from: MutPtr<Context>, to: MutPtr<Context>) {
	/* Safety: guaranteed by caller */
	unsafe {
		asm!(
			"bl {}",
			sym xx_core_fiber_arm64_switch,
			in("x0") from.as_mut_ptr(),
			in("x1") to.as_mut_ptr(),
			lateout("x18") _,
			lateout("x20") _,
			lateout("x21") _,
			lateout("x22") _,
			lateout("x23") _,
			lateout("x24") _,
			lateout("x25") _,
			lateout("x26") _,
			lateout("x27") _,
			lateout("x28") _,
			lateout("d8") _,
			lateout("d9") _,
			lateout("d10") _,
			lateout("d11") _,
			lateout("d12") _,
			lateout("d13") _,
			lateout("d14") _,
			lateout("d15") _,
			lateout("d16") _,
			lateout("d17") _,
			lateout("d18") _,
			lateout("d19") _,
			lateout("d20") _,
			lateout("d21") _,
			lateout("d22") _,
			lateout("d23") _,
			lateout("d24") _,
			lateout("d25") _,
			lateout("d26") _,
			lateout("d27") _,
			lateout("d28") _,
			lateout("d29") _,
			lateout("d30") _,
			lateout("d31") _,
			clobber_abi("C")
		);
	}
}
