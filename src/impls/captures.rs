pub trait Captures<'__> {}

impl<T: ?Sized> Captures<'_> for T {}
