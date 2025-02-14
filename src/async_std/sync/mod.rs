//! Implementations of async aware synchronization primitives. These
//! implementations suspend the current async worker instead of blocking the
//! thread
//!
//! See the individual modules below for more information
//!
//! [`channel::oneshot`]: send a single message to another task
//!
//! [`channel::mpmc`]: a highly efficient multi-producer multi-consumer queue
//!
//! [`channel::mpsc`]: the same as mpmc for now
//!
//! [`broadcast`]: broadcast all sent values to every receiver
//!
//! [`mutex`]: mutual exclusion
//!
//! [`notify`]: a simple wait list

use std::mem::{forget, MaybeUninit};
use std::result;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use super::*;
use crate::cell::UnsafeCell;
use crate::pointer::*;

mod wait_list;
use self::wait_list::*;

pub mod broadcast;
pub mod channel;
pub mod mutex;
pub mod notify;

#[doc(inline)]
pub use mutex::{Mutex, MutexGuard};
#[doc(inline)]
pub use {channel::*, notify::*};
