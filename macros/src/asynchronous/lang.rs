use super::*;

pub fn try_change_task_output(output: &mut ReturnType) {
	let ReturnType::Type(_, ty) = output else {
		return;
	};

	let Type::Path(TypePath { qself: None, path }) = ty.as_mut() else {
		return;
	};

	if path.leading_colon.is_none() &&
		path.segments.len() == 2 &&
		path.segments[0].ident == "Self" &&
		path.segments[0].arguments.is_none() &&
		path.segments[1].ident == "Output" &&
		path.segments[1].arguments.is_empty()
	{
		path.segments[1].arguments = PathArguments::AngleBracketed(parse_quote! { <'_> });
	}
}

pub fn try_change_task_type(generics: &mut Generics) {
	if generics.params.is_empty() {
		generics.params.push(parse_quote! { 'ctx });
	}
}

pub fn task_impl(
	mut attrs: AttributeArgs, use_lang: TokenStream, mut item: ItemTrait
) -> Result<TokenStream> {
	attrs.async_kind.0 = AsyncKind::Task;

	if let Some(TraitItem::Fn(func)) = item
		.items
		.iter_mut()
		.find(|item| matches!(item, TraitItem::Fn(_)))
	{
		try_change_task_output(&mut func.sig.output);

		let doc = transform_trait_func(
			&mut Function::from_trait_fn(true, None, func),
			&task_doc_fn,
			|func| transform_async(attrs, func)
		)?;

		item.items.push(TraitItem::Fn(doc));
	}

	if let Some(TraitItem::Type(ty)) = item
		.items
		.iter_mut()
		.find(|item| matches!(item, TraitItem::Type(_)))
	{
		try_change_task_type(&mut ty.generics);
	}

	Ok(quote! {
		const _: () = {
			#use_lang
		};

		#item
	})
}

pub fn task_wrap_impl(use_lang: TokenStream, item: ItemStruct) -> TokenStream {
	let ident = &item.ident;
	let context = Context::new();
	let context_ident = &context.ident;
	let not_doc_attr = not_doc_attr();

	quote! {
		#item

		const _: () = {
			use ::std::ops::FnOnce;
			use ::std::marker::PhantomData;
			use ::xx_core::coroutines::Context;

			#use_lang

			impl<F: FnOnce(&Context) -> Output, Output> #ident<F, Output> {
				pub const fn new(func: F) -> Self {
					Self(func, PhantomData)
				}
			}

			#not_doc_attr
			impl<F: FnOnce(&Context) -> Output, Output> Task for #ident<F, Output> {
				type Output<'ctx> = Output;

				#[inline]
				unsafe fn run(self, #context) -> Output {
					self.0(#context_ident)
				}
			}
		};
	}
}

pub fn async_closure_impl(use_lang: TokenStream, item: ItemStruct) -> TokenStream {
	let ident = &item.ident;

	quote! {
		#item

		const _: () = {
			use ::std::ops::FnOnce;
			use ::xx_core::coroutines::{asynchronous, get_context, Context};

			#use_lang

			impl<F, const T: usize> #ident<F, T> {
				pub const fn new(func: F) -> Self {
					Self(func)
				}
			}

			#[asynchronous(traitext)]
			impl<F: FnOnce(Args, &Context) -> Output, Args, Output> AsyncFnOnce<Args>
				for #ident<F, 0> {
				type Output = Output;

				#[inline]
				async fn call_once(self, args: Args) -> Output {
					self.0(args, get_context().await)
				}
			}

			#[asynchronous(traitfn)]
			impl<F: FnMut(Args, &Context) -> Output, Args, Output> AsyncFnMut<Args>
				for #ident<F, 1> {
				type Output = Output;

				#[inline]
				async fn call_mut(&mut self, args: Args) -> Output {
					self.0(args, get_context().await)
				}
			}

			#[asynchronous(traitfn)]
			impl<F: Fn(Args, &Context) -> Output, Args, Output> AsyncFn<Args>
				for #ident<F, 2> {
				type Output = Output;

				#[inline]
				async fn call(&self, args: Args) -> Output {
					self.0(args, get_context().await)
				}
			}
		};
	}
}
