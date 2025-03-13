use super::*;

#[asynchronous(traitext)]
pub trait AsyncFnOnce<Args = ()> {
	type Output;

	async fn call_once(self, args: Args) -> Self::Output;
}

#[asynchronous]
pub trait AsyncFnMut<Args = ()> {
	type Output;

	async fn call_mut(&mut self, args: Args) -> Self::Output;
}

#[asynchronous]
pub trait AsyncFn<Args = ()> {
	type Output;

	async fn call(&self, args: Args) -> Self::Output;
}
