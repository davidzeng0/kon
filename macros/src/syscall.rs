use super::*;

struct SyscallImpl {
	instruction: LitStr,
	out: String,
	num: String,
	regs: Vec<String>,
	clobber: Vec<String>
}

impl Parse for SyscallImpl {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let instruction: LitStr = input.parse::<LitStr>()?;

		input.parse::<Token![;]>()?;

		let mut out = None;
		let mut num = None;
		let mut args = Vec::new();
		let mut clobber = Vec::new();

		while !input.is_empty() {
			let kind = input.parse::<Ident>()?;

			input.parse::<Token![=]>()?;

			let regs = Punctuated::<Ident, Token![,]>::parse_separated_nonempty(input)?;
			let regs: Vec<_> = regs.iter().map(ToString::to_string).collect();

			input.parse::<Token![;]>()?;

			let kind_str: &str = &kind.to_string();

			if matches!(kind_str, "out" | "num") && regs.len() != 1 {
				let msg = format!("There can only be one `{}` register", kind_str);

				return Err(Error::new_spanned(kind, msg));
			}

			match kind_str {
				"out" => out = Some(regs[0].clone()),
				"num" => num = Some(regs[0].clone()),
				"arg" => args = regs,
				"clobber" => clobber = regs,

				_ => {
					let msg = "Unknown kind, expected one of `out`, `num`, `arg`, or `clobber`";

					return Err(Error::new_spanned(kind, msg));
				}
			}
		}

		let out = out.ok_or_else(|| input.error("Expected an output register `out = reg`"))?;
		let num = num.ok_or_else(|| input.error("Expected a number register `num = reg`"))?;

		Ok(Self { instruction, out, num, regs: args, clobber })
	}
}

impl SyscallImpl {
	fn expand(&self) -> TokenStream {
		let (instruction, out, num, regs, clobber) = (
			&self.instruction,
			&self.out,
			&self.num,
			&self.regs,
			&self.clobber
		);

		let mut functions = Vec::new();
		let args: Vec<_> = (0..regs.len()).map(|i| format_ident!("arg{}", i)).collect();

		for argc in 0..=regs.len() {
			let regs = &regs[0..argc];
			let args = &args[0..argc];
			let func = format_ident!("syscall{}", argc);

			functions.push(quote! {
				#[inline(always)]
				pub unsafe fn #func(num: i32, #(#args: usize),*) -> isize {
					let result;

					::std::arch::asm!(
						#instruction,
						in(#num) num,
						#(in(#regs) #args,)*
						lateout(#out) result,
						#(lateout(#clobber) _,)*
						options(nostack, preserves_flags)
					);

					result
				}
			});
		}

		quote! { #(#functions)* }
	}
}

pub fn syscall_impl(item: TokenStream) -> Result<TokenStream> {
	parse2::<SyscallImpl>(item).map(|syscall| syscall.expand())
}

struct ArrayOptions {
	len: Option<Expr>
}

fn parse_options(meta: &Meta) -> Result<ArrayOptions> {
	let mut options = ArrayOptions { len: None };

	let metas = match meta {
		Meta::Path(_) => return Ok(options),
		Meta::List(list) => {
			Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())?
		}

		Meta::NameValue(_) => {
			let msg = "Expected `#[array(option = value)]`";

			return Err(Error::new_spanned(meta, msg));
		}
	};

	for meta in metas {
		let Meta::NameValue(nv) = meta else {
			let msg = "Expected a `option` = `value` arg";

			return Err(Error::new_spanned(meta, msg));
		};

		if nv.path.require_ident()? != "len" {
			return Err(Error::new_spanned(nv.path, "Unknown option"));
		}

		options.len = Some(nv.value);
	}

	Ok(options)
}

fn make_pat_type(pat_ty: &PatType, new_ident: Ident, ty: impl FnOnce(Type) -> Type) -> PatType {
	let mut result = pat_ty.clone();

	let Pat::Ident(ident) = result.pat.as_mut() else {
		unreachable!();
	};

	ident.ident = new_ident;
	*result.ty = ty(*result.ty);
	result
}

#[allow(clippy::type_complexity)]
fn get_raw_args(
	args: &mut Punctuated<FnArg, Token![,]>
) -> Result<(
	Punctuated<FnArg, Token![,]>,
	Vec<Stmt>,
	Punctuated<Expr, Token![,]>
)> {
	let (mut raw_args, mut vars, mut into_raw) = (Punctuated::new(), Vec::new(), Punctuated::new());

	for arg in args {
		let FnArg::Typed(ty) = arg else {
			return Err(Error::new_spanned(arg, "Receiver is not allowed here"));
		};

		let Pat::Ident(pat_ident) = ty.pat.as_ref() else {
			return Err(Error::new_spanned(&ty.pat, "Pattern not allowed here"));
		};

		let array = ty.attrs.remove_any("array");
		let mut pat_ty = ty.clone();

		let Some(array) = array else {
			let (pat, ty) = (&pat_ty.pat, &pat_ty.ty);

			into_raw.push(parse_quote! { IntoRaw::into_raw(#pat) });
			pat_ty.ty = parse_quote_spanned! { ty.span() =>
				<#ty as ::xx_core::os::syscall::IntoRaw>::Raw
			};

			raw_args.push(FnArg::Typed(pat_ty));

			continue;
		};

		let ptr = make_pat_type(&pat_ty, format_ident!("{}_ptr", pat_ident.ident), |ty| {
			parse_quote_spanned! { ty.span() =>
				<#ty as ::xx_core::os::syscall::IntoRawArray>::Pointer
			}
		});

		let mut len = make_pat_type(&pat_ty, format_ident!("{}_len", pat_ident.ident), |ty| {
			parse_quote_spanned! { ty.span() =>
				<#ty as ::xx_core::os::syscall::IntoRawArray>::Length
			}
		});

		let options = parse_options(&array.meta)?;

		into_raw.push(parse_quote! { (#pat_ident).0 });

		if options.len.is_some() {
			into_raw.push(parse_quote! { TryInto::try_into((#pat_ident).1).unwrap() });
		} else {
			into_raw.push(parse_quote! { (#pat_ident).1 });
		}

		vars.push(parse_quote! {
			let #pat_ident = IntoRawArray::into_raw_array(#pat_ident);
		});

		if let Some(expr) = &options.len {
			*len.ty = parse_quote! { #expr };
		}

		raw_args.push(FnArg::Typed(ptr));
		raw_args.push(FnArg::Typed(len));
	}

	Ok((raw_args, vars, into_raw))
}

pub fn syscall_define(attrs: TokenStream, item: TokenStream) -> Result<TokenStream> {
	let number: Expr = parse2(attrs)?;
	let mut func: ForeignItemFn = parse2(item)?;

	let (raw_args, vars, into_raw) = get_raw_args(&mut func.sig.inputs)?;
	let (attrs, vis, sig) = (&func.attrs, &func.vis, &func.sig);
	let args: Vec<_> = raw_args.get_pats(false).into_iter().collect();

	let mut raw_sig = sig.clone();

	raw_sig.ident = format_ident!("{}_raw", raw_sig.ident);
	raw_sig.unsafety.get_or_insert_with(Default::default);
	raw_sig.inputs = raw_args;

	let raw_ident = &raw_sig.ident;

	Ok(quote! {
		#(#attrs)* #vis #raw_sig {
			let number = (#number) as i32;

			{
				use ::std::convert::{From, Into};
				use ::xx_core::os::syscall::{IntoRaw, IntoRawArray, SyscallParameter, syscall_raw};

				let result = unsafe {
					syscall_raw!(
						number
						#(, SyscallParameter::from(#args).0)*
					)
				};

				Into::into(SyscallResult(result))
			}
		}

		#(#attrs)* #vis #sig {
			use ::std::convert::TryInto;
			use ::xx_core::os::syscall::{IntoRaw, IntoRawArray};

			#(#vars)*

			unsafe { #raw_ident(#into_raw) }
		}
	})
}
