use super::*;
use crate::macros::{macro_each, paste};

#[asynchronous(traitext)]
pub trait TaskExt: Task + Sized {
	async fn map<F, V, O>(self, map: F) -> O
	where
		F: AsyncFnOnce(V) -> O,
		Self: for<'ctx> Task<Output<'ctx> = V>
	{
		map.call_once(self.await).await
	}

	async fn map_sync<F, V, O>(self, map: F) -> O
	where
		F: FnOnce(V) -> O,
		Self: for<'ctx> Task<Output<'ctx> = V>
	{
		map(self.await)
	}
}

impl<T: Task> TaskExt for T {}

macro_rules! async_fn {
	(([$($self:tt)*] [$async_kind:ident] $([$call:ident, $kind:tt])?)) => {
		paste! {
			impl<F, T, Args, Output> [< AsyncFn $($kind)? >] <Args> for F
			where
				F: [< Fn $($kind)? >] (Args) -> T,
				T: for<'ctx> Task<Output<'ctx> = Output>
			{
				type Output = Output;

				#[cfg(not(any(doc, feature = "xx-doc")))]
				#[asynchronous($async_kind)]
				async fn [< call $($call)? >](
					$($self)* self, args: Args
				) -> Self::Output {
					self(args).await
				}

				#[cfg(any(doc, feature = "xx-doc"))]
				async fn [< call $($call)? >](
					$($self)* self, args: Args
				) -> Self::Output {
					unreachable!();
				}
			}
		}
	};
}

macro_each!(async_fn, ([] [traitext] [_once, Once]), ([&mut] [traitfn] [_mut, Mut]), ([&] [traitfn]));

macro_rules! map_fn {
	(
		($($call:ident, $kind:tt)?)
	) => {
		paste! {
			pub trait [< AsyncFn $($kind)? Map >] <Args>: [< AsyncFn $($kind)? >] <Args> + Sized {
				#[cfg(not(any(doc, feature = "xx-doc")))]
				#[allow(unused_mut)]
				#[asynchronous(sync)]
				fn [< map $($call)? >] <F, Output>(
					mut self, mut map: F
				) -> impl [< AsyncFn $($kind)? >](Args) -> Output
				where
					F: [< AsyncFn $($kind)? >](Self::Output) -> Output
				{
					move |args: Args| async move {
						map. [< call $($call)? >] (
							self. [< call $($call)? >] (args).await
						).await
					}
				}

				#[cfg(any(doc, feature = "xx-doc"))]
				fn [< map $($call)? >] <F>(
					mut self, mut map: F
				) -> impl [< AsyncFn $($kind)? >] <Args, Output = F::Output>
				where
					F: [< AsyncFn $($kind)? >] <Self::Output>
				{
					internal::DocAsyncFn::new()
				}

				#[cfg(not(any(doc, feature = "xx-doc")))]
				#[allow(unused_mut)]
				#[asynchronous(sync)]
				fn [< map $($call)? _sync >] <F, Output>(
					mut self, mut map: F
				) -> impl [< AsyncFn $($kind)? >](Args) -> Output
				where
					F: [< Fn $($kind)? >](Self::Output) -> Output
				{
					move |args: Args| async move {
						map(self. [< call $($call)? >] (args).await)
					}
				}

				#[cfg(any(doc, feature = "xx-doc"))]
				fn [< map $($call)? _sync >] <F, Output>(
					mut self, mut map: F
				) -> impl [< AsyncFn $($kind)? >] <Args, Output = Output>
				where
					F: [< Fn $($kind)? >](Self::Output) -> Output
				{
					internal::DocAsyncFn::new()
				}
			}

			impl<F: [< AsyncFn $($kind)? >] <Args>, Args>
				[< AsyncFn $($kind)? Map >] <Args> for F {}
		}
	};
}

macro_each!(map_fn, (_once, Once), (_mut, Mut), ());
