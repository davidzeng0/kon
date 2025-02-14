use super::*;

fn not_allowed<T>(what: &Option<T>, message: &'static str) -> Result<()>
where
	T: ToTokens
{
	if let Some(tokens) = what {
		Err(Error::new_spanned(tokens, message))
	} else {
		Ok(())
	}
}

fn transform_last_arg(inputs: &mut Punctuated<FnArg, Token![,]>, return_type: &Type) -> Result<()> {
	let msg = "The last argument must be `request: _`";

	let Some(FnArg::Typed(req)) = inputs.last_mut() else {
		return Err(Error::new_spanned(inputs, msg));
	};

	let Pat::Ident(PatIdent { ident, subpat: None, .. }) = req.pat.as_ref() else {
		return Err(Error::new_spanned(&req.pat, msg));
	};

	if ident != "request" {
		return Err(Error::new_spanned(ident, msg));
	}

	if !matches!(req.ty.as_ref(), Type::Infer(_)) {
		return Err(Error::new_spanned(&req.ty, msg));
	}

	req.ty = parse_quote! { ::xx_core::future::ReqPtr<#return_type> };

	Ok(())
}

fn transform_func(func: &mut Function<'_>) -> Result<()> {
	if !func.is_root && func.attrs.remove_path("future").is_none() {
		return Ok(());
	}

	let return_type = func.sig.output.to_type();

	let mut cancel_closure_type = {
		let mut types = vec![quote! { ::xx_core::future::ReqPtr<#return_type> }];

		if let Some(rec) = func.sig.receiver() {
			let ty = &rec.ty;

			types.insert(0, quote! { #ty });
		}

		let default_cancel_capture = join_tuple(types);

		quote! { ::xx_core::future::internal::CancelClosure<#default_cancel_capture> }
	};

	let request = Ident::new("__req", Span::mixed_site());

	if let Some(block) = &mut func.block {
		for stmt in &mut block.stmts {
			let Stmt::Item(Item::Fn(cancel)) = stmt else {
				continue;
			};

			let Some(attr) = cancel.attrs.remove_path("cancel") else {
				continue;
			};

			let Visibility::Inherited = cancel.vis else {
				let msg = "Visibility not allowed here";

				return Err(Error::new_spanned(&cancel.vis, msg));
			};

			not_allowed(&cancel.sig.constness, "`const` not allowed here")?;
			not_allowed(&cancel.sig.asyncness, "`async` not allowed here")?;
			not_allowed(&cancel.sig.abi, "ABI not allowed here")?;
			not_allowed(&cancel.sig.generics.lt_token, "Generics not allowed here")?;
			not_allowed(&cancel.sig.variadic, "Variadics not allowed here")?;

			cancel.sig.inputs.push(parse_quote! {
				request: ::xx_core::future::ReqPtr<#return_type>
			});

			cancel_closure_type = make_explicit_closure(
				&mut Function {
					is_root: true,
					vis: None,
					attrs: &mut vec![],
					env_generics: func.env_generics,
					sig: &mut cancel.sig,
					block: Some(&mut cancel.block)
				},
				&[(quote! { () }, quote! { () })],
				quote! { ::xx_core::future::internal::CancelClosure },
				|capture, ret| {
					quote_spanned! { ret.span() =>
						::xx_core::future::internal::CancelClosure<#capture>
					}
				},
				Some(Annotations::Uniform)
			)?;

			cancel.sig.inputs.pop();

			ReplaceSelf.visit_item_fn_mut(cancel);

			let (attrs, unsafety, inputs, output, block) = (
				&cancel.attrs,
				&cancel.sig.unsafety,
				&cancel.sig.inputs,
				&cancel.sig.output,
				&cancel.block
			);

			let ident = format_ident!("{}", cancel.sig.ident, span = attr.span());
			let color = Ident::new("drop", cancel.sig.ident.span());

			*stmt = parse_quote! {
				#[allow(unused_variables)]
				#(#attrs)*
				let #ident = {
					const _: () = {
						let _ = ::std::mem::#color::<()>;
					};

					let request = #request;

					move | #inputs | #output #unsafety #block
				};
			};

			break;
		}

		block.stmts.insert(
			0,
			parse_quote! {
				let #request = request;
			}
		);
	}

	transform_last_arg(&mut func.sig.inputs, &return_type)?;

	func.sig.inputs.pop();
	func.sig.output = parse_quote_spanned! { return_type.span() =>
		-> ::xx_core::future::Progress<#return_type, #cancel_closure_type>
	};

	make_opaque_closure(
		func,
		&[(
			quote! { request },
			quote! { ::xx_core::future::ReqPtr<#return_type> }
		)],
		|_| {
			quote_spanned! { return_type.span() =>
				::xx_core::future::Progress<#return_type, #cancel_closure_type>
			}
		},
		OpaqueClosureType::Custom(|ret: TokenStream| {
			(
				quote_spanned! { ret.span() =>
					::xx_core::future::Future<Output = #return_type, Cancel = #cancel_closure_type>
				},
				quote! { ::xx_core::future::internal::FutureClosure }
			)
		}),
		Some(Annotations::default())
	)?;

	Ok(())
}

fn doc_fn(func: &mut Function<'_>) -> Result<TokenStream> {
	let mut attrs = func.attrs.clone();

	if !func.is_root && attrs.remove_path("future").is_none() {
		return default_doc(func);
	}

	let mut sig = func.sig.clone();
	let return_type = sig.output.to_type();

	transform_last_arg(&mut sig.inputs, &return_type)?;

	sig.inputs.pop();
	sig.output = parse_quote! {
		-> impl ::xx_core::future::Future<Output = #return_type>
	};

	let vis = &func.vis;

	let block = match &mut func.block {
		Some(block) => quote_spanned! { block.span() => {
			::xx_core::future::internal::get_future()
		}},

		None => quote_spanned! { func.sig.span() =>
			;
		}
	};

	Ok(quote! {
		#(#attrs)*
		#vis #sig
		#block
	})
}

pub fn future(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
	let item = parse2::<Functions>(item)?;

	attr.require_empty()?;

	item.transform_all(Some(&doc_fn), transform_func, |item| {
		!matches!(item, Functions::Trait(_) | Functions::TraitFn(_))
	})
}
