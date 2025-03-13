use super::*;

fn closure_lifetime<R>() -> R
where
	R: Parse
{
	parse_quote! { '__xx_iclslt }
}

fn capture_lifetimes(sig: &Signature, env_generics: Option<&Generics>) -> TokenStream {
	let mut addl_bounds = Punctuated::<TypeParamBound, Token![+]>::new();

	if let Some(generics) = env_generics {
		for param in generics.lifetimes() {
			let lifetime = &param.lifetime;

			addl_bounds.push(parse_quote! { ::xx_core::impls::captures::Captures<#lifetime> });
		}
	}

	/* See https://github.com/danielhenrymantilla/fix_hidden_lifetime_bug.rs
	 * TODO: remove when trait aliases are stable
	 */
	for param in sig.generics.lifetimes() {
		let lifetime = &param.lifetime;

		addl_bounds.push(parse_quote! { ::xx_core::impls::captures::Captures<#lifetime> });
	}

	addl_bounds.push(closure_lifetime());

	quote! { #addl_bounds }
}

fn add_lifetimes(
	sig: &mut Signature, env_generics: Option<&Generics>, annotations: Option<Annotations>
) -> TokenStream {
	let Some(annotations) = annotations else {
		return quote! {};
	};

	let closure_lt = closure_lifetime::<Lifetime>();
	let mut op = AddLifetime::new(closure_lt.clone(), annotations);

	op.visit_signature_mut(sig);

	if op.explicit_lifetimes.is_empty() && op.added_lifetimes.is_empty() {
		return quote! {};
	}

	let clause = sig
		.generics
		.where_clause
		.get_or_insert_with(WhereClause::default);

	let mut add_bounds = |params: &Punctuated<GenericParam, Token![,]>| {
		for param in params {
			match param {
				GenericParam::Const(_) => (),
				GenericParam::Type(ty) => {
					let ident = &ty.ident;

					clause.predicates.push(parse_quote! { #ident: #closure_lt });
				}

				GenericParam::Lifetime(param) => {
					let lifetime = &param.lifetime;

					clause
						.predicates
						.push(parse_quote! { #lifetime: #closure_lt });
				}
			}
		}
	};

	add_bounds(&sig.generics.params);

	if let Some(generics) = env_generics {
		add_bounds(&generics.params);
	}

	sig.generics.params.push(closure_lifetime());

	for lifetime in &op.added_lifetimes {
		sig.generics.params.push(parse_quote! { #lifetime });
	}

	for lifetime in op.added_lifetimes.iter().chain(&op.explicit_lifetimes) {
		clause
			.predicates
			.push(parse_quote! { #lifetime: #closure_lt });
	}

	let lifetimes = capture_lifetimes(sig, env_generics);

	quote! { + #lifetimes }
}

pub fn join_tuple<T>(data: Vec<T>) -> TokenStream
where
	T: ToTokens
{
	let types = quote! { #(#data),* };

	if data.len() == 1 {
		types
	} else {
		quote! { (#types) }
	}
}

fn build_tuples(
	inputs: &mut Punctuated<FnArg, Token![,]>, map: impl FnMut(&mut FnArg) -> (Type, Pat, Pat)
) -> (TokenStream, TokenStream, TokenStream) {
	let data: Vec<(Type, Pat, Pat)> = inputs.iter_mut().map(map).collect();

	(
		join_tuple(data.iter().map(|tp| tp.0.clone()).collect()),
		join_tuple(data.iter().map(|tp| tp.1.clone()).collect()),
		join_tuple(data.iter().map(|tp| tp.2.clone()).collect())
	)
}

fn make_args(args_pat_type: &[(TokenStream, TokenStream)]) -> (TokenStream, TokenStream) {
	let mut args = Vec::new();
	let mut types = Vec::new();

	for (pat, ty) in args_pat_type {
		args.push(quote! { #pat: #ty });
		types.push(ty);
	}

	(quote! { #(#args),* }, quote! { #(#types),* })
}

pub fn make_explicit_closure(
	func: &mut Function<'_>, args: &[(TokenStream, TokenStream)], closure_type: TokenStream,
	transform_return: impl Fn(TokenStream, TokenStream) -> TokenStream,
	annotations: Option<Annotations>
) -> Result<TokenStream> {
	add_lifetimes(func.sig, func.env_generics, annotations);

	let (types, construct, destruct) = build_tuples(&mut func.sig.inputs, |arg| match arg {
		FnArg::Typed(pat) => {
			let destr = pat.pat.as_ref().clone();

			RemoveModifiers.visit_pat_mut(pat.pat.as_mut());

			(pat.ty.as_ref().clone(), pat.pat.as_ref().clone(), destr)
		}

		FnArg::Receiver(rec) => {
			let make_pat_ident = |ident: Option<&str>, copy_mut: bool| {
				let mutability = if copy_mut && rec.reference.is_none() {
					rec.mutability
				} else {
					None
				};

				let ident = if let Some(ident) = ident {
					Ident::new(ident, Span::mixed_site())
				} else {
					Ident::new("self", rec.self_token.span())
				};

				Pat::Ident(PatIdent {
					attrs: rec.attrs.clone(),
					by_ref: None,
					mutability,
					ident,
					subpat: None
				})
			};

			(
				rec.ty.as_ref().clone(),
				make_pat_ident(None, false),
				make_pat_ident(Some(SELF_IDENT), true)
			)
		}
	});

	let (args, _) = make_args(args);
	let return_type = func.sig.output.to_type();
	let closure_return_type = transform_return(types.clone(), return_type.to_token_stream());

	func.sig.output = parse_quote! { -> #closure_return_type };

	if let Some(block) = &mut func.block {
		ReplaceSelf.visit_block_mut(block);

		let inline = func.attrs.remove_any("inline");
		let cls = quote_spanned! { block.span() => {
			#inline
			| #destruct: #types, #args | -> #return_type #block
		}};

		**block = parse_quote! {{
			#closure_type::new(
				#[allow(clippy::used_underscore_binding)]
				{ #construct },
				#cls
			)
		}};
	}

	Ok(closure_return_type)
}

#[allow(dead_code)]
pub enum OpaqueClosureType<T> {
	Fn(),
	Custom(T)
}

pub fn make_opaque_closure(
	func: &mut Function<'_>, args: &[(TokenStream, TokenStream)],
	transform_return: impl Fn(&mut Type) -> TokenStream,
	closure_type: OpaqueClosureType<impl Fn(TokenStream) -> (TokenStream, TokenStream)>,
	annotations: Option<Annotations>
) -> Result<TokenStream> {
	let addl_lifetimes = add_lifetimes(func.sig, func.env_generics, annotations);

	let mut return_type = func.sig.output.to_type();
	let closure_return_type = transform_return(&mut return_type);

	let (args, args_types) = make_args(args);

	let (closure_return_type, trait_impl_wrap) = match closure_type {
		OpaqueClosureType::Custom(transform) => {
			let (trait_type, trait_impl) = transform(closure_return_type);

			(
				parse_quote_spanned! { trait_type.span() =>
					impl #trait_type
				},
				Some(trait_impl)
			)
		}

		OpaqueClosureType::Fn() => (
			parse_quote_spanned! { closure_return_type.span() =>
				impl ::std::ops::FnOnce( #args_types ) -> #closure_return_type
			},
			None
		)
	};

	if let Some(block) = &mut func.block {
		let inline = func.attrs.remove_any("inline");
		let mut closure = quote! {{
			#inline
			move | #args | -> #return_type #block
		}};

		if let Some(wrap) = trait_impl_wrap {
			closure = quote! { #wrap::new(#closure) }
		}

		**block = parse_quote! {{ #closure }};
	}

	func.sig.output = parse_quote! { -> #closure_return_type #addl_lifetimes };
	func.attrs.push(parse_quote! {
		#[allow(clippy::type_complexity)]
	});

	Ok(closure_return_type)
}
