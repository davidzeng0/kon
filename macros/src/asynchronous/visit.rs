use super::*;

fn tuple_args(args: &mut Punctuated<Pat, Token![,]>) {
	let (mut pats, mut tys) = (Vec::new(), Vec::new());

	for input in take(args) {
		match input {
			Pat::Type(ty) => {
				pats.push(*ty.pat);
				tys.push(*ty.ty);
			}

			_ => {
				pats.push(input);
				tys.push(parse_quote! { _ });
			}
		}
	}

	let (pats, tys) = (join_tuple(pats), join_tuple(tys));

	args.push(Pat::Type(parse_quote! { #pats: #tys }));
}

pub struct TransformAsync(pub bool);

impl TransformAsync {
	fn visit_closure<F>(&mut self, visit: F)
	where
		F: FnOnce(&mut Self)
	{
		let has_await = self.0;

		visit(self);

		self.0 = has_await;
	}

	fn transform_async(&mut self, inner: &mut ExprAsync) -> Expr {
		self.visit_closure(|this| this.visit_expr_async_mut(inner));

		let (mut attrs, capture, block) = (take(&mut inner.attrs), &inner.capture, &inner.block);
		let inline = attrs.remove_any("inline");
		let context = Context::new();

		parse_quote! {
			#(#attrs)*
			::xx_core::coroutines::internal::as_task(
				::xx_core::coroutines::internal::OpaqueTask::new({
					#inline
					#capture |#context| #block
				})
			)
		}
	}

	fn transform_await(&mut self, inner: &mut ExprAwait) -> Expr {
		self.0 = true;
		self.visit_expr_await_mut(inner);

		let (attrs, base) = (&inner.attrs, &inner.base);
		let ident = Context::ident();

		parse_quote_spanned! { inner.await_token.span() =>
			#(#attrs)*
			::xx_core::coroutines::internal::unsafe_stub_do_not_use(#ident, #base)
		}
	}

	fn transform_closure(&mut self, closure: &mut ExprClosure) -> Expr {
		let body = closure.body.as_mut();
		let span;

		#[allow(clippy::never_loop)]
		loop {
			if let Some(asyncness) = closure.asyncness.take() {
				span = asyncness.span();

				if !matches!(body, Expr::Block(_)) {
					*body = parse_quote! {{ #body }};
				}

				break;
			}

			if let Expr::Async(expr) = body {
				if expr.capture.is_some() {
					span = expr.async_token.span();

					*body = Expr::Block(ExprBlock {
						attrs: expr.attrs.clone(),
						label: None,
						block: expr.block.clone()
					});

					break;
				}
			}

			TransformSync.visit_expr_closure_mut(closure);

			return Expr::Closure(closure.clone());
		}

		let context = Context::new();

		tuple_args(&mut closure.inputs);

		closure.inputs.push(Pat::Type(parse_quote! { #context }));

		self.visit_closure(|this| this.visit_expr_mut(body));

		let mut attrs = take(&mut closure.attrs);
		let inline = attrs.remove_any("inline");

		let closure = if let Some(inline) = inline {
			quote! {{
				#inline
				#closure
			}}
		} else {
			closure.to_token_stream()
		};

		parse_quote_spanned! { span =>
			#(#attrs)*
			::xx_core::coroutines::internal::OpaqueAsyncFn::new(#closure)
		}
	}
}

impl VisitMut for TransformAsync {
	fn visit_item_mut(&mut self, _: &mut Item) {}

	fn visit_expr_mut(&mut self, expr: &mut Expr) {
		*expr = match expr {
			Expr::Async(inner) => self.transform_async(inner),
			Expr::Await(inner) => self.transform_await(inner),
			Expr::Closure(inner) => self.transform_closure(inner),
			_ => return visit_expr_mut(self, expr)
		};
	}

	fn visit_macro_mut(&mut self, mac: &mut Macro) {
		visit_macro_body(self, mac);
	}
}

pub struct TransformSync;

impl VisitMut for TransformSync {
	fn visit_item_mut(&mut self, _: &mut Item) {}

	fn visit_expr_mut(&mut self, expr: &mut Expr) {
		let mut visit = TransformAsync(false);

		*expr = match expr {
			Expr::Async(inner) => visit.transform_async(inner),
			Expr::Closure(inner) => visit.transform_closure(inner),
			_ => return visit_expr_mut(self, expr)
		};
	}

	fn visit_macro_mut(&mut self, mac: &mut Macro) {
		visit_macro_body(self, mac);
	}
}

pub struct ReplaceLifetime<'a>(pub &'a Lifetime);

impl VisitMut for ReplaceLifetime<'_> {
	fn visit_lifetime_mut(&mut self, lt: &mut Lifetime) {
		if lt.ident == self.0.ident {
			lt.ident = Ident::new("_", lt.ident.span());
		}
	}
}

#[allow(clippy::missing_panics_doc)]
fn replace_async_fn(bound: &mut TypeParamBound) -> Result<()> {
	const IDENTS: &[&str] = &["AsyncFnOnce", "AsyncFnMut", "AsyncFn"];

	#[allow(clippy::never_loop)]
	loop {
		let TypeParamBound::Trait(TraitBound { lifetimes, path, .. }) = bound else {
			break;
		};

		let last = path.segments.last_mut().unwrap();

		if !IDENTS.contains(&last.ident.to_string().as_ref()) {
			break;
		}

		let PathArguments::Parenthesized(args) = &mut last.arguments else {
			let msg = "Expected parenthetical notation";

			return Err(Error::new_spanned(&last.arguments, msg));
		};

		let mut op = AddLifetime::new(parse_quote! { '__xx_hrlt }, Annotations::default());
		let mut tys = Vec::new();

		for mut ty in take(&mut args.inputs) {
			op.visit_type_mut(&mut ty);
			tys.push(ty);
		}

		let inputs = join_tuple(tys);
		let output = args.output.to_type();

		last.arguments = PathArguments::AngleBracketed(parse_quote! {
			<#inputs, Output = #output>
		});

		if op.added_lifetimes.is_empty() {
			break;
		}

		let bound = lifetimes.get_or_insert_with(Default::default);

		for lt in op.added_lifetimes {
			bound.lifetimes.push(parse_quote! { #lt });
		}

		break;
	}

	Ok(())
}

pub struct ReplaceAsyncFn(Option<Error>);

impl VisitMut for ReplaceAsyncFn {
	fn visit_type_param_bound_mut(&mut self, bound: &mut TypeParamBound) {
		visit_type_param_bound_mut(self, bound);

		if self.0.is_none() {
			self.0 = replace_async_fn(bound).err();
		}
	}
}

impl ReplaceAsyncFn {
	pub fn visit_sig(sig: &mut Signature) -> Result<()> {
		let mut this = Self(None);

		this.visit_signature_mut(sig);

		match this.0 {
			None => Ok(()),
			Some(err) => Err(err)
		}
	}
}
