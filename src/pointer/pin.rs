use super::*;

pub trait Pin {
	/// # Safety
	/// cannot call when already pinned
	unsafe fn pin(&mut self) {}
}

pub struct Pinned<P>(P);

impl<P> Pinned<P> {
	#[must_use]
	pub const fn new(pointer: P) -> Self {
		Self(pointer)
	}

	/// # Safety
	/// the implementation specific contract for unpinning P must be satisfied
	pub unsafe fn into_inner(self) -> P {
		self.0
	}
}

impl<P: Deref> Deref for Pinned<P> {
	type Target = P::Target;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<P: DerefMut> DerefMut for Pinned<P> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<P: Clone> Clone for Pinned<P> {
	fn clone(&self) -> Self {
		Self::new(self.0.clone())
	}
}

sealed_trait!(trait Pin);

pub trait PinExt: PinSealed {
	/// # Safety
	/// Pinning a mutable reference allows for easy unpinning.
	///
	/// See [`Pinned::into_inner`]
	unsafe fn pin_local(&mut self) -> Pinned<&mut Self> {
		let mut pinned = Pinned::new(self);

		/* Safety: we are being pinned */
		unsafe { pinned.pin() };

		pinned
	}

	fn pin_box(self) -> Pinned<Box<Self>>
	where
		Self: Sized
	{
		let mut this = Pinned::new(Box::new(self));

		/* Safety: we are being pinned */
		unsafe { this.pin() };

		this
	}

	#[allow(clippy::unwrap_used)]
	fn pin_rc(self) -> Pinned<Rc<Self>>
	where
		Self: Sized
	{
		let mut rc = Rc::new(self);
		let this = Rc::get_mut(&mut rc).unwrap();

		/* Safety: we are being pinned */
		unsafe { this.pin() };

		Pinned::new(rc)
	}

	#[allow(clippy::unwrap_used)]
	fn pin_arc(self) -> Pinned<Arc<Self>>
	where
		Self: Sized
	{
		let mut arc = Arc::new(self);
		let this = Arc::get_mut(&mut arc).unwrap();

		/* Safety: we are being pinned */
		unsafe { this.pin() };

		Pinned::new(arc)
	}
}

impl<T: PinSealed> PinExt for T {}

#[macro_export]
macro_rules! pin {
	($x:ident) => {
		let mut $x = $x;

		#[allow(unused_mut)]
		/* Safety: original variable is shadowed, so it cannot be moved */
		let mut $x = unsafe { $crate::pointer::PinExt::pin_local(&mut $x) };
	};
}

pub use pin;
