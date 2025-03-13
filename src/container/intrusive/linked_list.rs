#![allow(clippy::multiple_unsafe_ops_per_block)]
//! Intrusive linked list implementation

use super::*;
use crate::cell::Cell;
use crate::macros::assert_unsafe_precondition;
use crate::opt::hint::assume;

/// The node of a linked list
pub struct Node {
	prev: Cell<Ptr<Self>>,
	next: Cell<Ptr<Self>>
}

impl Node {
	/// # Safety
	/// all pointers are pinned must live as long as they are linked
	unsafe fn set_ptrs(this: Ptr<Self>, prev: Option<Ptr<Self>>, next: Option<Ptr<Self>>) {
		/* Safety: guaranteed by caller */
		unsafe {
			if let Some(prev) = prev {
				ptr!(this=>prev.set(prev));
				ptr!(prev=>next.set(this));
			}

			if let Some(next) = next {
				ptr!(this=>next.set(next));
				ptr!(next=>prev.set(this));
			}
		}
	}

	/// # Safety
	/// Must be pinned and not be linked
	unsafe fn set_circular(this: Ptr<Self>) {
		/* Safety: guaranteed by caller */
		unsafe { Self::set_ptrs(this, Some(this), None) };
	}

	/// Create a new default node
	#[must_use]
	pub const fn new() -> Self {
		Self {
			prev: Cell::new(Ptr::null()),
			next: Cell::new(Ptr::null())
		}
	}

	/// Returns `true` if the node is linked in a list
	pub fn linked(&self) -> bool {
		/*
		 * Safety: if we are linked, both must be non-null, or both must be null
		 * May help with optimizations
		 */
		unsafe { assume(self.prev.get().is_null() == self.next.get().is_null()) };

		!self.prev.get().is_null()
	}

	/// Unlink the node, asserting that it is already linked
	///
	/// # Safety
	/// This node must be valid and linked
	pub unsafe fn unlink_unchecked(&self) {
		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(self.linked()) };

		let (prev, next) = (
			self.prev.replace(Ptr::null()),
			self.next.replace(Ptr::null())
		);

		/* Safety: prev and next must live as long as they are linked */
		unsafe {
			ptr!(prev=>next.set(next));
			ptr!(next=>prev.set(prev));
		}
	}

	/// Unlink the node
	///
	/// # Safety
	/// This node must be valid
	///
	/// # Panics
	/// If this node isn't linked
	pub unsafe fn unlink(&self) {
		assert!(self.linked());

		/* Safety: guaranteed by caller. we just ensured we are linked */
		unsafe { self.unlink_unchecked() };
	}
}

impl Default for Node {
	fn default() -> Self {
		Self::new()
	}
}

/// An intrusive linked list
pub struct LinkedList {
	base: Node
}

impl LinkedList {
	fn base(&self) -> Ptr<Node> {
		ptr!(&self.base)
	}

	/// Create a new empty and unpinned list
	#[must_use]
	pub const fn new() -> Self {
		Self { base: Node::new() }
	}

	/// Returns `true` if the list is empty
	///
	/// If the list isn't pinned, the return value is unspecified behavior
	pub fn is_empty(&self) -> bool {
		self.base() == self.head()
	}

	/// Get the head pointer of the list
	///
	/// The list should be pinned and a check to [`is_empty`] should be done
	/// before using the pointer
	///
	/// [`is_empty`]: Self::is_empty
	pub fn head(&self) -> Ptr<Node> {
		let head = self.base.next.get();

		/*
		 * Safety: the node is in our list
		 *
		 * calling head().unlink() should optimize away the linked check,
		 * also aborts in debug if the condition isn't satisfied
		 */
		unsafe { assume(ptr!(head=>linked())) };

		head
	}

	/// Get the tail pointer of the list
	///
	/// The list should be pinned and a check to [`is_empty`] should be done
	/// before using the pointer
	///
	/// [`is_empty`]: Self::is_empty
	pub fn tail(&self) -> Ptr<Node> {
		let tail = self.base.next.get();

		/*
		 * Safety: the node is in our list
		 *
		 * calling tail().unlink() should optimize away the linked check
		 * also aborts in debug if the condition isn't satisfied
		 */
		unsafe { assume(ptr!(tail=>linked())) };

		tail
	}

	/// # Safety
	/// same as remove, and node must not be base if the list isn't empty
	unsafe fn pop_edge(&self, node: Ptr<Node>) -> Option<Ptr<Node>> {
		if !self.is_empty() {
			/* Safety: guaranteed by caller */
			unsafe { self.remove(node) };

			Some(node)
		} else {
			None
		}
	}

	/// Pop a node from the front of the list. Returns `None` if the list is
	/// empty
	///
	/// # Safety
	/// the list must be pinned
	pub unsafe fn pop_front(&self) -> Option<Ptr<Node>> {
		/* Safety: we are removing a node from our list */
		unsafe { self.pop_edge(self.head()) }
	}

	/// Pop a node from the back of the list. Returns `None` if the list is
	/// empty
	///
	/// # Safety
	/// the list must be pinned
	pub unsafe fn pop_back(&self) -> Option<Ptr<Node>> {
		/* Safety: we are removing a node from our list */
		unsafe { self.pop_edge(self.tail()) }
	}

	/// # Safety
	/// This list must be pinned, Node must be pinned and live while it's in the
	/// list, and not already be in a list
	pub unsafe fn append(&self, node: Ptr<Node>) {
		/* Safety: node is only ever added to one list at a time, so we have
		 * exclusive access to the node. The list now has the right to create a
		 * mutable borrow to the inner, so we cannot expect a &mut Node in the
		 * function signature. Base and prev must live as long as they are linked */
		unsafe { Node::set_ptrs(node, Some(self.base.prev.get()), Some(self.base())) };
	}

	/// # Safety
	/// This list must be pinned, Node must be pinned and linked to this list
	pub unsafe fn remove(&self, node: Ptr<Node>) {
		/* Safety: guaranteed by caller */
		unsafe { node.as_ref().unlink_unchecked() };
	}

	/// # Safety
	/// The new list must be pinned, empty, and live as long as it has nodes
	pub unsafe fn move_elements(&self, other: &Self) {
		/* Safety: guaranteed by caller */
		unsafe { assert_unsafe_precondition!(other.is_empty()) };

		if self.is_empty() {
			return;
		}

		let (prev, next) = (self.base.prev.get(), self.base.next.get());

		/* Safety: all nodes must live as long as they are linked */
		unsafe { Node::set_ptrs(other.base(), Some(prev), Some(next)) };

		/* Safety: we're now empty */
		unsafe { Node::set_circular(self.base()) };
	}
}

impl Pin for LinkedList {
	unsafe fn pin(&mut self) {
		/* Safety: we are being pinned, which means there is nothing in the list */
		unsafe { Node::set_circular(self.base()) };
	}
}

impl Default for LinkedList {
	fn default() -> Self {
		Self::new()
	}
}
