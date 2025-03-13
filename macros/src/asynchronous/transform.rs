use super::*;

pub struct Context {
	pub ident: TokenStream,
	pub ty: TokenStream
}

impl Context {
	pub fn new() -> Self {
		Self { ident: Self::ident(), ty: Self::ty() }
	}

	pub fn ident() -> TokenStream {
		quote_spanned! { Span::mixed_site() => context }
	}

	pub fn ty() -> TokenStream {
		quote! { &::xx_core::coroutines::Context }
	}
}

impl ToTokens for Context {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.ident.to_tokens(tokens);

		quote! { : }.to_tokens(tokens);

		self.ty.to_tokens(tokens);
	}
}

fn task_impl(attrs: &[Attribute], ident: &Ident) -> TokenStream {
	if attrs.is_empty() {
		quote! {
			type XXInternalAsyncSupportWrap<F, Output> =
				::xx_core::coroutines::internal::OpaqueTask<F, Output>;
		}
	} else {
		let new = Ident::new("new", ident.span());
		let run = Ident::new("run", ident.span());

		quote! {
			struct XXInternalAsyncSupportWrap<F, Output>(F, ::std::marker::PhantomData<Output>);

			const _: () = {
				use ::std::ops::FnOnce;
				use ::std::marker::PhantomData;
				use ::xx_core::coroutines::{Context, Task};

				impl<F: FnOnce(&Context) -> Output, Output> XXInternalAsyncSupportWrap<F, Output> {
					pub const fn #new(func: F) -> Self {
						Self(func, PhantomData)
					}
				}

				impl<F: FnOnce(&Context) -> Output, Output> Task
					for XXInternalAsyncSupportWrap<F, Output> {
					type Output<'ctx> = Output;

					#(#attrs)*
					#[inline]
					unsafe fn #run(self, context: &Context) -> Output {
						self.0(context)
					}
				}
			};
		}
	}
}

fn lending_task_impl(lt: &Lifetime, ident: &Ident, output: &Type) -> TokenStream {
	let mut ret = output.clone();
	let context = Context::ty();
	let new = Ident::new("new", ident.span());
	let run = Ident::new("run", ident.span());

	ReplaceLifetime(lt).visit_type_mut(&mut ret);

	quote! {
		struct XXInternalAsyncSupportWrap<F>(F);

		const _: () = {
			use ::std::ops::FnOnce;
			use ::std::marker::PhantomData;
			use ::xx_core::coroutines::{Context, Task};

			impl<F: FnOnce(&Context) -> #ret> XXInternalAsyncSupportWrap<F> {
				pub const fn #new(func: F) -> Self {
					Self(func)
				}
			}

			impl<F: FnOnce(&Context) -> #ret> Task for XXInternalAsyncSupportWrap<F> {
				type Output<#lt> = #output;

				#[inline]
				unsafe fn #run(self, context: #context) -> #ret {
					self.0(context)
				}
			}
		};
	}
}

pub fn transform_sync(func: &mut Function<'_>) -> Result<()> {
	if let Some(block) = &mut func.block {
		TransformSync.visit_block_mut(block);
	}

	Ok(())
}

fn get_run_attrs(attrs: &mut Vec<Attribute>) -> Vec<Attribute> {
	let targets = ["must_use", "cold"];
	let mut removed = Vec::new();

	for target in targets {
		while let Some(attr) = attrs.remove_any(target) {
			removed.push(attr);
		}
	}

	removed
}

fn get_cx_lifetime(generics: &mut Generics) -> Result<Option<Lifetime>> {
	let mut context_lifetime = None;

	for param in &mut generics.params {
		let GenericParam::Lifetime(param) = param else {
			continue;
		};

		let Some(attr) = param.attrs.remove_path("cx") else {
			continue;
		};

		if context_lifetime.is_some() {
			return Err(Error::new_spanned(attr, "Duplicate context lifetime"));
		}

		context_lifetime = Some(param.lifetime.clone());
	}

	Ok(context_lifetime)
}

fn impl_lang(lang: Lang, span: Span, func: &mut Function<'_>) -> Result<()> {
	let Some(block) = &mut func.block else {
		#[allow(clippy::needless_borrows_for_generic_args)]
		return Err(Error::new_spanned(&func.sig, "An empty block is required"));
	};

	if !block.stmts.is_empty() {
		return Err(Error::new_spanned(block, "This block must be empty"));
	}

	block.stmts.push(parse_quote_spanned! { span =>
		#[allow(unused_imports)]
		use ::xx_core::coroutines::lang;
	});

	let imp = match lang {
		Lang::GetContext => {
			let context = Context::ident();

			parse_quote! { #context }
		}
		_ => unreachable!()
	};

	block.stmts.push(Stmt::Expr(imp, None));

	Ok(())
}

pub fn transform_async(mut attrs: AttributeArgs, func: &mut Function<'_>) -> Result<()> {
	let Some(asyncness) = func.sig.asyncness.take() else {
		return if !func.is_root {
			transform_sync(func)
		} else {
			let message = "The `async` keyword is missing from the function declaration";

			Err(Error::new_spanned(func.sig.fn_token, message))
		};
	};

	attrs.parse_attrs(func.attrs)?;

	let closure_type = attrs.async_kind.0.closure_type();
	let func_attrs = get_run_attrs(func.attrs);

	let mut cx_lt = get_cx_lifetime(&mut func.sig.generics)?;

	if let Some((lang, span)) = attrs.language {
		if let Some(lifetime) = &cx_lt {
			let msg = "Context lifetime forbidden in lang items";

			return Err(Error::new_spanned(lifetime, msg));
		}

		impl_lang(lang, span, func)?;
	}

	let for_lt = if let Some(lifetime) = &cx_lt {
		if func.sig.generics.params.len() != 1 {
			let msg = "Generics not allowed here";

			return Err(Error::new_spanned(&func.sig.generics, msg));
		}

		func.sig.generics.params.clear();

		if !matches!(closure_type, ClosureType::Default | ClosureType::Trait) {
			let msg = "Unsupported closure type for this operation";

			return Err(Error::new(attrs.async_kind.1, msg));
		}

		lifetime.clone()
	} else if matches!(attrs.language, Some((Lang::GetContext, _))) {
		let lt: Lifetime = parse_quote! { 'current };

		cx_lt = Some(lt.clone());
		lt
	} else {
		parse_quote! { '__xx_icurctx }
	};

	if let Some(block) = &mut func.block {
		let mut visit = TransformAsync(false);

		visit.visit_block_mut(block);

		if !visit.0 && attrs.language.is_none() && attrs.async_kind.0 != AsyncKind::TraitFn {
			let warning = parse_quote_spanned! { asyncness.span() =>
				const _: () = { async fn warning() {} };
			};

			block.stmts.insert(0, warning);
		}
	}

	let context = Context::new();
	let return_type = func.sig.output.to_type();

	if closure_type == ClosureType::None {
		func.sig.inputs.push(parse_quote! { #context });

		if func.sig.unsafety.is_some() {
			return Ok(());
		}

		/* caller must ensure we're allowed to suspend */
		func.sig.unsafety = Some(Default::default());

		if func.block.is_some() {
			func.attrs.push(parse_quote! {
				#[deny(unsafe_op_in_unsafe_fn)]
			});
		}

		return Ok(());
	}

	let map_return_type = |rt: &mut Type| {
		let return_type = rt.to_token_stream();

		if let Some(lt) = &cx_lt {
			ReplaceLifetime(lt).visit_type_mut(rt);
		}

		return_type
	};

	let impl_type = |rt: TokenStream| {
		(
			quote_spanned! { rt.span() =>
				for<#for_lt> ::xx_core::coroutines::Task<Output<#for_lt> = #rt>
			},
			quote! { XXInternalAsyncSupportWrap }
		)
	};

	let annotations = if closure_type != ClosureType::Trait && attrs.language.is_none() {
		Some(Annotations::default())
	} else {
		None
	};

	make_opaque_closure(
		func,
		&[(context.ident, context.ty)],
		map_return_type,
		OpaqueClosureType::Custom(impl_type),
		annotations
	)?;

	if let Some(block) = &mut func.block {
		let task_impl = if let Some(lt) = &cx_lt {
			lending_task_impl(lt, &func.sig.ident, &return_type)
		} else {
			task_impl(&func_attrs, &func.sig.ident)
		};

		let stmts = &block.stmts;

		**block = parse_quote! {{
			#task_impl
			#(#stmts)*
		}};
	}

	Ok(())
}

fn doc(kind: AsyncKind, func: &mut Function<'_>) -> Result<TokenStream> {
	ReplaceAsyncFn::visit_sig(func.sig)?;

	if !func.is_root && func.sig.asyncness.is_none() {
		return default_doc(func);
	}

	let (mut attrs, mut sig) = (func.attrs.clone(), func.sig.clone());
	let lang = get_lang(&mut attrs)?;

	#[allow(clippy::single_match)]
	match lang {
		Some((Lang::GetContext, _)) => sig.generics.params.push(parse_quote! { 'current }),
		_ => ()
	}

	for attr in &mut sig.generics.params {
		if let GenericParam::Lifetime(param) = attr {
			param.attrs.remove_path("cx");
		}
	}

	let clause = sig
		.generics
		.where_clause
		.get_or_insert_with(WhereClause::default);

	match kind {
		AsyncKind::Task => sig.output = parse_quote! { -> Self::Output<'static> },
		AsyncKind::TraitFn => {
			clause.predicates.push(parse_quote! {
				Self: xx_core::coroutines::internal::DocDynSafe
			});
		}

		_ => ()
	}

	let (vis, block) = (&func.vis, doc_block(func));

	Ok(quote! {
		#(#attrs)*
		#vis #sig
		#block
	})
}

pub fn task_doc_fn(func: &mut Function<'_>) -> Result<TokenStream> {
	doc(AsyncKind::Task, func)
}

pub fn traits_doc_fn(func: &mut Function<'_>) -> Result<TokenStream> {
	doc(AsyncKind::TraitFn, func)
}

pub fn sync_doc_fn(func: &mut Function<'_>) -> Result<TokenStream> {
	doc(AsyncKind::Sync, func)
}

fn impl_for_task(item: &mut Functions) -> Result<TokenStream> {
	let Functions::Impl(imp) = item else {
		return Err(Error::new_spanned(item, "Unexpected declaration"));
	};

	let Some((_, path, _)) = imp.trait_.as_ref() else {
		return Err(Error::new_spanned(imp, "Missing trait"));
	};

	if let Some(ImplItem::Fn(func)) = imp
		.items
		.iter_mut()
		.find(|item| matches!(item, ImplItem::Fn(func) if func.sig.ident == "run"))
	{
		if let Some(unsafety) = func.sig.unsafety {
			let msg = "Function must be safe to call";

			return Err(Error::new_spanned(unsafety, msg));
		}

		try_change_task_output(&mut func.sig.output);
	}

	if let Some(ImplItem::Type(ty)) = imp
		.items
		.iter_mut()
		.find(|item| matches!(item, ImplItem::Type(_)))
	{
		try_change_task_type(&mut ty.generics);
	}

	let ty = quote_spanned! { path.span() =>
		impl #path
	};

	Ok(quote! {
		const _: () = {
			const fn type_check(task: impl ::xx_core::coroutines::Task) -> #ty {
				task
			}
		};
	})
}

pub fn transform_items(mut item: Functions, attrs: AttributeArgs) -> Result<TokenStream> {
	let type_check = if attrs.async_kind.0 == AsyncKind::Task {
		Some(impl_for_task(&mut item)?)
	} else {
		None
	};

	let transformed = item.transform_all(
		Some(&|func| doc(attrs.async_kind.0, func)),
		|func| transform_async(attrs, func),
		|_| true
	)?;

	Ok(quote! {
		#transformed
		#type_check
	})
}
