use std::marker::PhantomData;
use std::mem::transmute;

use crate::pointer::*;

pub struct Closure<Capture, Args, Output> {
	capture: Capture,
	call: fn(Capture, Args) -> Output
}

impl<Capture, Args, Output> Closure<Capture, Args, Output> {
	pub const fn new(capture: Capture, call: fn(Capture, Args) -> Output) -> Self {
		Self { capture, call }
	}

	#[inline(always)]
	pub fn call(self, args: Args) -> Output {
		(self.call)(self.capture, args)
	}
}

pub struct OpaqueClosure<F, Args, Output>(F, PhantomData<(Args, Output)>);

impl<F: FnOnce(Args) -> Output, Args, Output> OpaqueClosure<F, Args, Output> {
	pub const fn new(func: F) -> Self {
		Self(func, PhantomData)
	}
}

impl<F: FnOnce(Args) -> Output, Args, Output> OpaqueClosure<F, Args, Output> {
	#[inline(always)]
	pub fn call(self, args: Args) -> Output {
		self.0(args)
	}
}

pub struct FnCallOnce<F, Args, Output> {
	func: Option<F>,
	phantom: PhantomData<(Args, Output)>
}

impl<F: FnOnce(Args) -> Output, Args, Output> FnCallOnce<F, Args, Output> {
	pub const fn new(func: F) -> Self {
		Self { func: Some(func), phantom: PhantomData }
	}

	/// # Panics
	/// If this function has already been called
	#[allow(clippy::expect_used, clippy::multiple_unsafe_ops_per_block)]
	pub fn as_dyn(&mut self) -> DynFnOnce<'_, Args, Output> {
		self.func.as_ref().expect("Function already called");

		/* Safety: func is Some as verified above */
		let call_once = |func: MutNonNull<()>, args| unsafe {
			ptr!(func.cast::<Option<F>>()=>take().unwrap_unchecked())(args)
		};

		DynFnOnce {
			func: ptr!(!null &mut self.func).cast(),
			call_once,
			phantom: PhantomData
		}
	}

	/// Get a `DynFnOnce` with an unbounded lifetime, effectively making it
	/// equivalent to a pointer
	///
	/// # Safety
	/// Pointer aliasing rules apply
	pub unsafe fn as_ptr<'a>(&mut self) -> DynFnOnce<'a, Args, Output> {
		/* Safety: guaranteed by caller */
		unsafe { transmute(self.as_dyn()) }
	}
}

pub struct DynFnOnce<'a, Args, Output> {
	func: MutNonNull<()>,
	call_once: unsafe fn(MutNonNull<()>, Args) -> Output,
	phantom: PhantomData<&'a ()>
}

impl<Args, Output> DynFnOnce<'_, Args, Output> {
	/// # Safety
	/// cannot call more than once
	pub unsafe fn call_once_ref(&self, args: Args) -> Output {
		let Self { func, call_once, .. } = *self;

		/* Safety: guaranteed by caller */
		unsafe { (call_once)(func, args) }
	}

	pub fn call_once(self, args: Args) -> Output {
		/* Safety: we have ownership of self so this is only called once */
		unsafe { self.call_once_ref(args) }
	}
}
