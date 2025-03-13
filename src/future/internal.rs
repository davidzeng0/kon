use super::*;
use crate::closure::*;

pub type FutureClosure<F, Output, Cancel> =
	OpaqueClosure<F, ReqPtr<Output>, Progress<Output, Cancel>>;

/* Safety: contract upheld by user of #[future] */
unsafe impl<F: FnOnce(ReqPtr<Output>) -> Progress<Output, C>, Output, C: Cancel> Future
	for FutureClosure<F, Output, C>
{
	type Cancel = C;
	type Output = Output;

	#[inline(always)]
	unsafe fn run(self, request: ReqPtr<Output>) -> Progress<Output, C> {
		self.call(request)
	}
}

pub type CancelClosure<Capture> = Closure<Capture, (), Result<()>>;

/* Safety: contract upheld by user of #[future] */
unsafe impl<Capture> Cancel for CancelClosure<Capture> {
	#[inline(always)]
	unsafe fn run(self) -> Result<()> {
		self.call(())
	}
}

#[cfg(any(doc, feature = "xx-doc"))]
pub fn get_future<Output>() -> impl Future<Output = Output> {
	use std::marker::PhantomData;

	struct Fut<Output>(PhantomData<Output>);

	unsafe impl<Output> Future for Fut<Output> {
		type Cancel = ();
		type Output = Output;

		unsafe fn run(self, request: ReqPtr<Output>) -> Progress<Output, ()> {
			unreachable!();
		}
	}

	#[doc(hidden)]
	unsafe impl Cancel for () {
		unsafe fn run(self) -> Result<()> {
			unreachable!();
		}
	}

	Fut(PhantomData)
}
