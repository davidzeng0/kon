use std::mem::take;

use super::*;

pub struct Function<'a> {
	pub is_root: bool,
	pub vis: Option<&'a mut Visibility>,
	pub attrs: &'a mut Vec<Attribute>,
	pub env_generics: Option<&'a Generics>,
	pub sig: &'a mut Signature,
	pub block: Option<&'a mut Block>
}

impl<'a> Function<'a> {
	pub fn from_item_fn(
		is_root: bool, env_generics: Option<&'a Generics>, func: &'a mut ImplItemFn
	) -> Self {
		Self {
			is_root,
			vis: Some(&mut func.vis),
			attrs: &mut func.attrs,
			env_generics,
			sig: &mut func.sig,
			block: Some(&mut func.block)
		}
	}

	pub fn from_trait_fn(
		is_root: bool, env_generics: Option<&'a Generics>, func: &'a mut TraitItemFn
	) -> Self {
		Self {
			is_root,
			vis: None,
			attrs: &mut func.attrs,
			env_generics,
			sig: &mut func.sig,
			block: func.default.as_mut()
		}
	}
}

#[derive(Clone)]
pub enum Functions {
	Fn(ImplItemFn),
	TraitFn(TraitItemFn),
	Trait(ItemTrait),
	Impl(ItemImpl)
}

impl ToTokens for Functions {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Self::Fn(func) => func.to_tokens(tokens),
			Self::TraitFn(func) => func.to_tokens(tokens),
			Self::Impl(item) => item.to_tokens(tokens),
			Self::Trait(item) => item.to_tokens(tokens)
		}
	}
}

impl Parse for Functions {
	fn parse(item: ParseStream<'_>) -> Result<Self> {
		let lookahead = item.fork();

		lookahead.call(Attribute::parse_outer)?;
		lookahead.parse::<Visibility>()?;
		lookahead.parse::<Option<Token![default]>>()?;
		lookahead.parse::<Option<Token![unsafe]>>()?;

		if lookahead.peek(Token![auto]) || lookahead.peek(Token![trait]) {
			return item.parse().map(Self::Trait);
		}

		if lookahead.peek(Token![impl]) {
			return item.parse().map(Self::Impl);
		}

		lookahead.parse::<Option<Token![const]>>()?;
		lookahead.parse::<Option<Token![async]>>()?;
		lookahead.parse::<Option<Token![unsafe]>>()?;
		lookahead.parse::<Option<Abi>>()?;

		if lookahead.parse::<Token![fn]>().is_err() {
			return Err(item.error("Expected a function, trait, or impl"));
		}

		if let Ok(item) = item.parse() {
			return Ok(Self::Fn(item));
		}

		item.parse().map(Self::TraitFn)
	}
}

pub fn doc_attr() -> Attribute {
	parse_quote! { #[cfg(any(doc, feature = "xx-doc"))] }
}

pub fn not_doc_attr() -> Attribute {
	parse_quote! { #[cfg(not(any(doc, feature = "xx-doc")))] }
}

pub fn doc_block(func: &Function<'_>) -> TokenStream {
	match &func.block {
		Some(block) => quote_spanned! { block.span() => {
			::std::unreachable!()
		}},

		None => quote_spanned! { func.sig.span() =>
			;
		}
	}
}

pub fn default_doc(func: &mut Function<'_>) -> Result<TokenStream> {
	let (attrs, vis, sig) = (&func.attrs, &func.vis, &func.sig);
	let block = doc_block(func);

	Ok(quote! {
		#(#attrs)*
		#vis #sig
		#block
	})
}

type DocFn<'a> = &'a dyn Fn(&mut Function<'_>) -> Result<TokenStream>;

#[allow(clippy::missing_panics_doc)]
pub fn transform_func<T>(
	func: &mut Function<'_>, docs: DocFn<'_>, callback: T
) -> Result<ImplItemFn>
where
	T: Fn(&mut Function<'_>) -> Result<()>
{
	let doc = docs(func)?;

	callback(func)?;
	func.attrs.push(not_doc_attr());

	let mut doc_fn = parse2::<ImplItemFn>(doc).unwrap();

	doc_fn.attrs.push(doc_attr());

	Ok(doc_fn)
}

#[allow(clippy::missing_panics_doc)]
pub fn transform_trait_func<T>(
	func: &mut Function<'_>, docs: DocFn<'_>, callback: T
) -> Result<TraitItemFn>
where
	T: Fn(&mut Function<'_>) -> Result<()>
{
	let doc = docs(func)?;

	callback(func)?;
	func.attrs.push(not_doc_attr());

	let mut doc_fn = parse2::<TraitItemFn>(doc).unwrap();

	doc_fn.attrs.push(doc_attr());

	Ok(doc_fn)
}

impl Functions {
	pub fn transform_all<T, A>(
		self, docs: Option<DocFn<'_>>, callback: T, allowed: A
	) -> Result<TokenStream>
	where
		T: Fn(&mut Function<'_>) -> Result<()> + Copy,
		A: FnOnce(&Self) -> bool
	{
		if !allowed(&self) {
			return Err(Error::new_spanned(self, "Unexpected declaration"));
		}

		let docs = docs.unwrap_or(&default_doc);

		Ok(match self {
			Self::Fn(mut func) => {
				let doc = transform_func(
					&mut Function::from_item_fn(true, None, &mut func),
					docs,
					callback
				)?;

				quote! {
					#func
					#doc
				}
			}

			Self::TraitFn(mut func) => {
				let doc = transform_trait_func(
					&mut Function::from_trait_fn(true, None, &mut func),
					docs,
					callback
				)?;

				quote! {
					#func
					#doc
				}
			}

			Self::Impl(mut item) => {
				for impl_item in take(&mut item.items) {
					let ImplItem::Fn(mut func) = impl_item else {
						item.items.push(impl_item);

						continue;
					};

					let doc_fn = transform_func(
						&mut Function::from_item_fn(false, Some(&item.generics), &mut func),
						docs,
						callback
					)?;

					item.items.push(ImplItem::Fn(func));
					item.items.push(ImplItem::Fn(doc_fn));
				}

				item.to_token_stream()
			}

			Self::Trait(mut item) => {
				for trait_item in take(&mut item.items) {
					let TraitItem::Fn(mut func) = trait_item else {
						item.items.push(trait_item);

						continue;
					};

					let doc_fn = transform_trait_func(
						&mut Function::from_trait_fn(false, Some(&item.generics), &mut func),
						docs,
						callback
					)?;

					item.items.push(TraitItem::Fn(func));
					item.items.push(TraitItem::Fn(doc_fn));
				}

				item.to_token_stream()
			}
		})
	}
}
