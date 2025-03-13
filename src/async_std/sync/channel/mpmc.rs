//! A highly efficient multi-producer, multi-consumer queue for sending many
//! messages between many async tasks.
//!
//! Create a channel using the [`bounded()`] function, which returns a
//! [`Sender`] and [`Receiver`] pair used to send and receive the value,
//! respectively.
//!
//! The [`Sender`]s and [`Receiver`]s can be cloned to send to other async
//! tasks.
//!
//! # Example
//!
//! ```
//! let (tx, rx) = mpsc::bounded(4);
//!
//! spawn(async move {
//! 	println!("{:?}", rx.recv().await);
//! })
//! .await;
//!
//! tx.send("hello world").await;
//! ```
//!
//! When all [`Sender`]s have been dropped and after the remaining messages in
//! the channel have been received, further calls to [`Receiver::recv`] will
//! return [`RecvError::Closed`]
//!
//! When all [`Receiver`]s have been dropped, further calls to [`Sender::send`]
//! will return [`SendError::Closed`]
//!
//! This channel does not guarantee that all sent messages will be received by a
//! receiver when the channel is closed

pub use super::error::*;
use super::*;

channel_impl!(MCChannel, "mpsc");

impl<T> Clone for Receiver<T> {
	fn clone(&self) -> Self {
		Self::new(self.channel.clone())
	}
}
