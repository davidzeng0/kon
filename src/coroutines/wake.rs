use super::*;

#[derive(Clone, Copy)]
pub struct WakerVTable {
	prepare: unsafe fn(Ptr<()>),
	wake: unsafe fn(Ptr<()>, ReqPtr<()>)
}

impl WakerVTable {
	/// # Safety
	/// `prepare` must never unwind
	/// `wake` is thread safe and must never unwind
	#[must_use]
	pub const unsafe fn new(
		prepare: unsafe fn(Ptr<()>), wake: unsafe fn(Ptr<()>, ReqPtr<()>)
	) -> Self {
		Self { prepare, wake }
	}
}

#[allow(missing_copy_implementations)]
pub struct Waker {
	ptr: Ptr<()>,
	vtable: &'static WakerVTable
}

impl Waker {
	#[must_use]
	pub const fn new(ptr: Ptr<()>, vtable: &'static WakerVTable) -> Self {
		Self { ptr, vtable }
	}

	/// # Safety
	/// TBD
	pub unsafe fn prepare(&self) {
		/* Safety: guaranteed by caller */
		unsafe { (self.vtable.prepare)(self.ptr) };
	}

	/// # Safety
	/// Must have already called `prepare`
	/// Must only call once when it is ready to wake the task
	pub unsafe fn wake(&self, request: ReqPtr<()>) {
		/* Safety: guaranteed by caller */
		unsafe { (self.vtable.wake)(self.ptr, request) }
	}
}
