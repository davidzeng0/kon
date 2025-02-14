use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit_mut::*;
use xx_macros::*;
use xx_proc_macros::*;

mod asynchronous;
mod duration;
mod error;
mod future;
mod make_closure;
mod syscall;
mod transform;
mod visit;
mod wrap_function;

use self::make_closure::*;
use self::transform::*;
use self::visit::*;

declare_attribute_macro! {
	/// Desugar an async function or trait or their body
	///
	/// ```
	/// #[asynchronous]
	/// async fn parse_config() -> Result<Parsed> {
	/// 	let data = load_file().await;
	///
	/// 	parse(data)
	/// }
	///
	/// #[asynchronous]
	/// pub trait MyAsyncTrait {
	/// 	fn print(&self);
	///
	/// 	async fn do_stuff(&mut self);
	/// }
	///
	/// #[asynchronous(sync)]
	/// fn async_block() -> impl Task {
	/// 	async move {
	/// 		println!("hello world");
	/// 	}
	/// }
	/// ```
	pub fn asynchronous(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
		asynchronous::asynchronous(attr, item)
	}
}

declare_proc_macro! {
	/// Select from multiple async tasks, running the handler
	/// for the task that finishes first and cancelling the rest
	///
	/// ```
	/// let item = select! {
	/// 	get_env();
	///
	/// 	item = channel.recv() => {
	/// 		println!("{}", item);
	///
	/// 		Some(item)
	/// 	}
	///
	/// 	expire = sleep(duration!(5 s)) => {
	/// 		println!("got nothing");
	///
	/// 		None
	/// 	}
	/// }
	/// .await;
	/// ```
	pub fn select(item: TokenStream) -> Result<TokenStream> {
		asynchronous::branch::select(item)
	}
}

declare_proc_macro! {
	/// Join multiple async tasks, waiting for all of them to complete
	///
	/// ```
	/// let (a, b) = join! {
	/// 	get_env();
	///
	/// 	load_file("a.txt"),
	/// 	load_file("b.txt")
	/// }
	/// .await;
	/// ```
	pub fn join(item: TokenStream) -> Result<TokenStream> {
		asynchronous::branch::join(item)
	}
}

declare_attribute_macro! {
	/// Desugar a function to a Future
	///
	/// ```
	/// #[future]
	/// fn start_op(&self, arg: Type, request: _) -> Output {
	/// 	#[cancel]
	/// 	fn cancel_op(&self) -> Result<()> {
	/// 		// runs when the future is cancelled
	/// 		// request is captured from outside
	/// 		self.ops.remove(request);
	/// 	}
	///
	/// 	// runs when the future is started
	/// 	self.ops.insert(request, arg);
	///
	/// 	Progress::Pending(cancel_op(self))
	/// }
	/// ```
	pub fn future(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
		future::future(attr, item)
	}
}

declare_proc_macro! {
	/// Macro for generating wrapper function calls
	///
	/// ```
	/// wrapper_functions! {
	/// 	// optional. the expr to use for &self
	/// 	inner = <expr>;
	///
	/// 	// optional. the expr to use for &mut self
	/// 	mut inner = <expr>;
	///
	/// 	<vis> fn my_func<'a, B>(&self, c: &'a B) -> Output;
	///
	/// 	// automatically detects chains for builders
	/// 	<vis> alias = fn set_value(&mut self, value: i32) -> &mut Self;
	/// }
	/// ```
	///
	/// # Examples
	///
	/// ```
	/// #[derive(Clone, Copy)]
	/// pub struct WrappingI32(i32);
	///
	/// impl WrappingI32 {
	/// 	wrapper_functions! {
	/// 		inner = self.0;
	///
	/// 		pub fn checked_add(self, value: i32) -> Option<i32>;
	/// 		pub fn add = fn wrapping_add(self, value: i32) -> Self;
	/// 	}
	/// }
	/// ```
	pub fn wrapper_functions(item: TokenStream) -> Result<TokenStream> {
		wrap_function::wrapper_functions(item)
	}
}

declare_proc_macro! {
	/// Generate raw syscall stubs
	///
	/// ```
	/// syscall_impl! {
	/// 	"instruction";
	///
	/// 	// output register
	/// 	out = reg;
	/// 	// sycall number register
	/// 	num = reg;
	/// 	// arg registers in order
	/// 	arg = reg1, reg2, reg3, ...;
	/// 	// clobbers
	/// 	clobber = reg1, reg2;
	/// }
	/// ```
	///
	/// # Examples
	///
	/// ```
	/// syscall_impl! {
	/// 	"syscall";
	///
	/// 	out = rax;
	/// 	num = rax;
	/// 	arg = rdi, rsi, rdx, r10, r8, r9;
	/// 	clobber = rcx, r11;
	/// }
	///
	/// syscall_impl! {
	/// 	"svc 0";
	///
	/// 	out = x0;
	/// 	num = x8;
	/// 	arg = x0, x1, x2, x3, x4, x5;
	/// }
	/// ```
	pub fn syscall_impl(item: TokenStream) -> Result<TokenStream> {
		syscall::syscall_impl(item)
	}
}

declare_attribute_macro! {
	/// Define a typed syscall stub
	///
	/// ```
	/// #[syscall_define(number)]
	/// pub fn syscall_name(args: Type) -> OsResult<Output>;
	/// ```
	///
	/// # Example
	///
	/// ```
	/// const OPEN: i32 = 2;
	///
	/// #[syscall_define(OPEN)]
	/// pub fn open(filename: &CStr, flags: u32, mode: u32) -> OsResult<OwnedFd>;
	/// ```
	///
	/// The macro defines two functions. One with the original signature,
	/// and a raw, unsafe version with less constraints
	///
	/// ```
	/// pub unsafe fn open_raw(filename: Ptr<CStr>, flags: u32, mode: u32) -> OsResult<OwnedFd>;
	/// ```
	pub fn syscall_define(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
		syscall::syscall_define(attr, item)
	}
}

declare_proc_macro! {
	/// Create a duration from a human readable format
	///
	/// # Examples
	///
	/// ```
	/// assert_eq!(duration!(1 hour 5 minutes), Duration::from_secs(3900));
	/// assert_eq!(duration!(2 min 50 sec), Duration::from_secs(170));
	/// assert_eq!(duration!(12 : 25), Duration::from_secs(745));
	/// assert_eq!(duration!(9::52::27), Duration::from_secs(35547));
	/// ```
	pub fn duration(item: TokenStream) -> Result<TokenStream> {
		duration::duration(item)
	}
}

declare_attribute_macro! {
	/// Auto generates [`Debug`] and [`Display`] implementations
	///
	/// ```
	/// #[errors]
	/// pub enum MyErrors {
	/// 	#[display("Parse error: {}", f0)]
	/// 	#[kind = ErrorKind::InvalidInput]
	/// 	ParseError(parse::Error),
	///
	/// 	#[display("Cancelled")]
	/// 	#[kind = ErrorKind::Interrupted]
	/// 	Cancelled,
	///
	/// 	#[display("In progress")]
	/// 	InProgress // defaults to ErrorKind::Other
	/// }
	///
	/// #[errors(?Debug + ?Display)]
	/// pub enum SendError<T> {
	/// 	#[display("Channel closed")]
	/// 	Closed(T),
	///
	/// 	#[display("Channel full")]
	/// 	Full(T)
	/// }
	/// ```
	///
	/// [`Debug`]: std::fmt::Debug
	/// [`Display`]: std::fmt::Display
	pub fn errors(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
		error::errors(attr, item)
	}
}
