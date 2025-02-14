#[cfg(feature = "async_std")]
pub mod async_std;
#[cfg(feature = "cell")]
pub mod cell;
#[cfg(feature = "closure")]
pub mod closure;
#[cfg(feature = "container")]
pub mod container;
#[cfg(feature = "coroutines")]
pub mod coroutines;
#[cfg(feature = "error")]
pub mod error;
#[cfg(feature = "fiber")]
pub mod fiber;
#[cfg(feature = "future")]
pub mod future;
#[cfg(feature = "impls")]
pub mod impls;
#[cfg(feature = "io")]
pub mod io;
#[cfg(feature = "log")]
pub mod log;
#[cfg(feature = "macros")]
pub mod macros;
#[cfg(feature = "opt")]
pub mod opt;
#[cfg(feature = "os")]
pub mod os;
#[cfg(feature = "pointer")]
pub mod pointer;
#[cfg(feature = "runtime")]
pub mod runtime;
#[cfg(feature = "sync")]
pub mod sync;
#[cfg(feature = "threadpool")]
pub mod threadpool;

extern crate self as xx_core;

#[cfg(feature = "ctor")]
pub extern crate ctor;
#[cfg(feature = "enumflags2")]
pub extern crate enumflags2;
#[cfg(feature = "lazy_static")]
pub extern crate lazy_static;
#[cfg(feature = "memchr")]
pub extern crate memchr;
#[cfg(feature = "num-traits")]
pub extern crate num_traits;
