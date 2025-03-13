//! See [`mpmc`] for more information.
//!
//! For now, this is equivalent to an [`mpmc`] but the [`Receiver`] cannot be
//! cloned.

pub use super::error::*;
use super::*;

channel_impl!(MCChannel, "mpsc");
