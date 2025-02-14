use super::*;

fn format_fn_ident(ident: &Ident) -> Ident {
	format_ident!("__xx_iatfni_{}", ident)
}

fn format_trait_ident(ident: &Ident) -> Ident {
	format_ident!("{}Ext", ident)
}

fn wrapper_function(func: &mut TraitItemFn, this: &TokenStream) {
	let mut call = Vec::new();
	let ident = format_fn_ident(&func.sig.ident);

	call.push(quote! { #this::#ident });

	if !func.sig.generics.params.is_empty() {
		call.push(func.sig.generics.to_types_turbofish());
	}

	let mut args = func.sig.inputs.get_pats(true);

	if func.sig.asyncness.is_some() {
		let context = Context::ident();

		args.push(parse_quote! { #context });
	}

	call.push(quote! { (#args) });

	func.default = Some(parse_quote! {{ unsafe { #(#call)* } }});
	func.attrs.push(parse_quote! {
		#[allow(clippy::used_underscore_binding, clippy::unused_async)]
	});

	RemoveModifiers.visit_signature_mut(&mut func.sig);
}

fn trait_ext(mut attrs: AttributeArgs, mut ext: ItemTrait) -> Result<TokenStream> {
	let name = ext.ident;

	ext.ident = format_trait_ident(&name);

	let (_, type_generics, where_clause) = ext.generics.split_for_impl();
	let super_trait: TypeParamBound = parse_quote! { #name #type_generics };

	attrs.async_kind.0 = AsyncKind::TraitExt;

	let this = quote! { <Self as #super_trait> };

	for trait_item in take(&mut ext.items) {
		let TraitItem::Fn(mut func) = trait_item else {
			continue;
		};

		wrapper_function(&mut func, &this);

		let doc = transform_trait_func(
			&mut Function::from_trait_fn(false, Some(&ext.generics), &mut func),
			&traits_doc_fn,
			|func| transform_async(attrs, func)
		)?;

		ext.items.push(TraitItem::Fn(func));
		ext.items.push(TraitItem::Fn(doc));
	}

	let ident = &ext.ident;
	let sealed_ident = format_ident!("{}Sealed", name);
	let sealed_mod = format_ident!("__private_seal_{}", name);

	let generics = &ext.generics;
	let mut sealed_generics = ext.generics.clone();

	sealed_generics.params.push(parse_quote! {
		XXInternalTraitImplementer: #super_trait + ?Sized
	});

	ext.supertraits.push(parse_quote! {
		#sealed_mod::#sealed_ident #type_generics
	});

	let (sealed_generics, ..) = sealed_generics.split_for_impl();

	let mut ext_generics = ext.generics.clone();

	ext_generics.params.push(parse_quote! {
		XXInternalTraitImplementer: #sealed_mod::#sealed_ident #type_generics + ?Sized
	});

	let (new_generics, ..) = ext_generics.split_for_impl();
	let not_doc_attr = not_doc_attr();

	Ok(quote! {
		#[allow(non_snake_case)]
		mod #sealed_mod {
			pub trait #sealed_ident #generics : super::#super_trait {}
		}

		#not_doc_attr
		impl #sealed_generics #sealed_mod::#sealed_ident #type_generics for XXInternalTraitImplementer #where_clause {}

		#[cfg(not(doc))]
		#ext

		#not_doc_attr
		impl #new_generics #ident #type_generics for XXInternalTraitImplementer #where_clause {}
	})
}

fn async_impl_fn(attrs: AttributeArgs, func: &mut Function<'_>) -> Result<()> {
	func.is_root = false;
	func.sig.ident = format_fn_ident(&func.sig.ident);
	func.attrs.push(parse_quote! { #[doc(hidden)] });

	transform_async(attrs, func)
}

macro_rules! assign_default {
	($item:ident, $this:ident) => {
		let ident = &$item.ident;

		$item.default = Some((Default::default(), parse_quote! { #$this::#ident }));
	};
}

fn gen_impl<F>(imp: &ItemTrait, mut attrs: AttributeArgs, type_map: F) -> Result<TokenStream>
where
	F: FnOnce(TokenStream) -> TokenStream
{
	let name = &imp.ident;
	let (_, type_generics, where_clause) = imp.generics.split_for_impl();

	attrs.async_kind.0 = AsyncKind::TraitFn;

	let this = quote! { XXInternalTraitImplementer };
	let mut items = Vec::new();

	for trait_item in &imp.items {
		match trait_item.clone() {
			TraitItem::Const(mut item) => {
				assign_default!(item, this);

				items.push(TraitItem::Const(item));
			}

			TraitItem::Type(mut item) => {
				assign_default!(item, this);

				items.push(TraitItem::Type(item));
			}

			TraitItem::Fn(mut func) => {
				wrapper_function(&mut func, &this);

				let doc = transform_trait_func(
					&mut Function::from_trait_fn(false, Some(&imp.generics), &mut func),
					&traits_doc_fn,
					|func| async_impl_fn(attrs, func)
				)?;

				items.push(TraitItem::Fn(func));
				items.push(TraitItem::Fn(doc));
			}

			_ => ()
		}
	}

	let mut new_generics = imp.generics.clone();

	new_generics.params.push(parse_quote! {
		XXInternalTraitImplementer: #name #type_generics + ?Sized
	});

	let ty = type_map(quote! { XXInternalTraitImplementer });

	Ok(quote! {
		impl #new_generics #name #type_generics for #ty #where_clause {
			#(#items)*
		}
	})
}

fn gen_impls(attrs: AttributeArgs, item: &ItemTrait) -> Result<TokenStream> {
	let mut impls = Vec::<TokenStream>::new();

	if let Some(impl_gen) = &attrs.impl_gen {
		if impl_gen.impl_ref.is_some() {
			impls.push(gen_impl(item, attrs, |ty| quote! { &#ty })?);
		}

		if impl_gen.impl_mut.is_some() {
			impls.push(gen_impl(item, attrs, |ty| quote! { &mut #ty })?);
		}

		if impl_gen.impl_box.is_some() {
			impls.push(gen_impl(
				item,
				attrs,
				|ty| quote! { ::std::boxed::Box<#ty> }
			)?);
		}
	}

	Ok(quote! { #(#impls)* })
}

pub fn async_trait(mut attrs: AttributeArgs, item: ItemTrait) -> Result<TokenStream> {
	let ext = trait_ext(attrs, item.clone())?;
	let impls = gen_impls(attrs, &item)?;

	attrs.async_kind.0 = AsyncKind::TraitFn;

	let functions = Functions::Trait(item);
	let item = functions.transform_all(
		Some(&traits_doc_fn),
		|func| async_impl_fn(attrs, func),
		|_| true
	)?;

	Ok(quote! { #item #ext #impls })
}

pub fn async_impl(mut attrs: AttributeArgs, item: Functions) -> Result<TokenStream> {
	attrs.async_kind.0 = AsyncKind::TraitFn;

	item.transform_all(
		Some(&traits_doc_fn),
		|func| async_impl_fn(attrs, func),
		|item| matches!(item, Functions::Fn(_) | Functions::Impl(_))
	)
}
