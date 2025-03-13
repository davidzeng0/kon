use super::*;

define_context! {
	pub struct Context {
		r12: usize,
		r13: usize,
		r14: usize,
		r15: usize,
		rip: usize,
		rsp: usize,
		rbx: usize,
		rbp: usize,
	}
}

global_asm!(include_str!("x64.s"));

extern "C" {
	fn xx_core_fiber_x64_start();
	fn xx_core_fiber_x64_intercept();
	fn xx_core_fiber_x64_switch(from: MutPtr<Context>, to: MutPtr<Context>);
}

impl Context {
	pub const fn program_counter(&self) -> Ptr<()> {
		Ptr::from_addr(self.rip)
	}

	/// # Safety
	/// the fiber must not be running
	pub unsafe fn set_stack(&mut self, stack: Ptr<()>, len: usize) {
		#[allow(clippy::arithmetic_side_effects)]
		(self.rsp = stack.addr() + len);
	}

	/// # Safety
	/// the fiber must not be running
	pub unsafe fn set_start(&mut self, start: Start) {
		let stack = MutPtr::<Start>::from_addr(self.rsp);

		/* Safety: guaranteed by caller */
		unsafe { stack.sub(1).write(start) };

		self.rip = xx_core_fiber_x64_start as usize;
	}

	/// # Safety
	/// valid intercept
	pub unsafe fn set_intercept(&mut self, intercept: Intercept) {
		let stack = MutPtr::<Intercept>::from_addr(self.rsp);

		/* Safety: guaranteed by caller */
		unsafe { stack.sub(1).write(intercept) };

		self.rip = xx_core_fiber_x64_intercept as usize;
	}
}

/// # Safety
/// `from` is the current running worker
/// `to` is valid
pub unsafe fn switch(from: MutPtr<Context>, to: MutPtr<Context>) {
	/* Safety: guaranteed by caller */
	unsafe { xx_core_fiber_x64_switch(from, to) };
}
