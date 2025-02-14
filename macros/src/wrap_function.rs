use super::*;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Chain {
	None,
	Ref,
	Owned
}

struct Function {
	attrs: Vec<Attribute>,
	ident: Ident,
	vis: Visibility,
	sig: Signature,
	chain: Chain
}

struct WrapperFunctions {
	inner: Expr,
	inner_mut: Expr,
	functions: Vec<Function>
}

fn get_chain(sig: &Signature) -> Chain {
	let Some(receiver) = sig.receiver() else {
		return Chain::None;
	};

	if *receiver.ty != sig.output.to_type() {
		return Chain::None;
	}

	if receiver.reference.is_some() {
		return Chain::Ref;
	}

	if let Type::Path(TypePath { qself: None, path }) = receiver.ty.as_ref() {
		if matches!(path.get_ident(), Some(ident) if ident == "Self") {
			return Chain::Owned;
		}
	}

	Chain::None
}

impl Parse for WrapperFunctions {
	#[allow(clippy::unwrap_in_result)]
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let mut inner = None;
		let mut inner_mut = None;

		for _ in 0..2 {
			#[allow(clippy::nonminimal_bool)]
			if !input.peek(Ident) && !(input.peek(Token![mut]) && input.peek2(Ident)) {
				break;
			}

			let mutability: Option<Token![mut]> = input.parse()?;
			let ident: Ident = input.parse()?;

			#[allow(clippy::needless_late_init)]
			let rhs: Expr;

			if ident != "inner" {
				return Err(Error::new_spanned(ident, "Expected `inner`"));
			}

			input.parse::<Token![=]>()?;
			rhs = input.parse()?;
			input.parse::<Token![;]>()?;

			if mutability.is_some() {
				inner_mut = Some(rhs);
			} else {
				inner = Some(rhs);
			}
		}

		let (inner, inner_mut) = match (inner, inner_mut) {
			(Some(inner), Some(inner_mut)) => (inner, inner_mut),
			(Some(inner), None) | (None, Some(inner)) => (inner.clone(), inner),
			(None, None) => return Err(input.error("Expected an inner expression"))
		};

		let mut functions = Vec::new();

		while !input.is_empty() {
			let attrs = input.call(Attribute::parse_outer)?;

			let vis: Visibility = input.parse()?;
			let mut sig: Signature;
			let mut ident: Option<Ident> = None;

			if let Ok(fn_token) = input.parse::<Token![fn]>() {
				let sig_ident: Ident = input.parse()?;

				ident = Some(sig_ident.clone());

				if input.parse::<Option<Token![=]>>()?.is_some() {
					sig = input.parse()?;
				} else {
					let mut generics: Generics = input.parse()?;

					let content;
					let paren_token = parenthesized!(content in input);
					let inputs = Punctuated::<FnArg, Token![,]>::parse_terminated(&content)?;
					let output = input.parse()?;

					generics.where_clause = input.parse()?;
					sig = Signature {
						constness: None,
						asyncness: None,
						unsafety: None,
						abi: None,
						fn_token,
						ident: sig_ident,
						generics,
						paren_token,
						inputs,
						variadic: None,
						output
					};
				}
			} else {
				sig = input.parse()?;
			}

			let ident = ident.unwrap_or_else(|| sig.ident.clone());

			input.parse::<Token![;]>()?;

			let chain = get_chain(&sig);

			if chain == Chain::Owned {
				let Some(FnArg::Receiver(receiver)) = sig.inputs.first_mut() else {
					unreachable!();
				};

				receiver.mutability = Some(Default::default());
			}

			functions.push(Function { attrs, ident, vis, sig, chain });
		}

		Ok(Self { inner, inner_mut, functions })
	}
}

impl WrapperFunctions {
	fn expand(&self) -> TokenStream {
		let mut fns = Vec::new();

		for func in &self.functions {
			let mut call = Vec::new();

			let inner = if func
				.sig
				.receiver()
				.is_some_and(|rec| rec.mutability.is_some())
			{
				&self.inner_mut
			} else {
				&self.inner
			};

			call.push(quote_spanned! { func.sig.span() => (#inner) });

			let ident = &func.sig.ident;
			let pats = func.sig.inputs.get_pats(false);

			call.push(quote_spanned! { func.sig.span() => .#ident });

			if !func.sig.generics.params.is_empty() {
				call.push(func.sig.generics.to_types_turbofish());
			}

			call.push(quote_spanned! { pats.span() => (#pats) });

			if func.sig.asyncness.is_some() {
				call.push(quote_spanned! { func.sig.span() => .await });
			}

			let mut sig = func.sig.clone();
			let mut attrs = func.attrs.clone();

			attrs.push(parse_quote! { #[inline(always)] });
			attrs.push(parse_quote! { #[allow(unsafe_op_in_unsafe_fn)] });
			sig.ident = func.ident.clone();

			let body = match func.chain {
				Chain::None => quote! { #(#call)* },
				Chain::Ref => quote! { #(#call)*; self },
				Chain::Owned => quote! { (#inner) = #(#call)*; self }
			};

			fns.push(ItemFn {
				attrs,
				vis: func.vis.clone(),
				sig,
				block: parse_quote! {{ #body }}
			});
		}

		quote! { #(#fns)* }
	}
}

pub fn wrapper_functions(item: TokenStream) -> Result<TokenStream> {
	parse2::<WrapperFunctions>(item).map(|functions| functions.expand())
}
