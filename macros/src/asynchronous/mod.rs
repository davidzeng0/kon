use std::mem::take;

use super::*;

pub mod branch;
mod lang;
mod options;
mod traits;
mod transform;
mod visit;

use self::lang::*;
use self::options::*;
use self::traits::*;
use self::transform::*;
use self::visit::*;

#[derive(Clone)]
pub enum AsyncItem {
	Fn(ImplItemFn),
	TraitFn(TraitItemFn),
	Trait(ItemTrait),
	Impl(ItemImpl),
	Struct(ItemStruct)
}

impl Parse for AsyncItem {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let lookahead = input.fork();

		lookahead.call(Attribute::parse_outer)?;
		lookahead.parse::<Visibility>()?;

		if lookahead.peek(Token![struct]) {
			return input.parse().map(Self::Struct);
		}

		Ok(match Functions::parse(input)? {
			Functions::Fn(item) => Self::Fn(item),
			Functions::TraitFn(item) => Self::TraitFn(item),
			Functions::Trait(item) => Self::Trait(item),
			Functions::Impl(item) => Self::Impl(item)
		})
	}
}

#[allow(clippy::missing_panics_doc)]
fn language_impl(mut attrs: AttributeArgs, item: AsyncItem) -> Result<TokenStream> {
	let (lang, span) = attrs.language.take().unwrap();

	let use_lang = quote_spanned! { span =>
		#[allow(unused_imports)]
		use ::xx_core::coroutines::lang;
	};

	Ok(match (lang, item) {
		(Lang::TaskWrap, AsyncItem::Struct(item)) => task_wrap_impl(use_lang, item),
		(Lang::AsyncClosure, AsyncItem::Struct(item)) => async_closure_impl(use_lang, item),
		(Lang::Task, AsyncItem::Trait(task)) => task_impl(attrs, use_lang, task)?,
		_ => return Err(Error::new(span, "Invalid language item"))
	})
}

pub fn asynchronous(attrs: TokenStream, item: TokenStream) -> Result<TokenStream> {
	let mut attrs = AttributeArgs::parse.parse2(attrs)?;
	let mut item = parse2::<AsyncItem>(item)?;

	if let Some(gen) = attrs.impl_gen {
		if !matches!(
			(&item, attrs.async_kind.0),
			(AsyncItem::Trait(_), AsyncKind::Implicit)
		) {
			return Err(Error::new(gen.span, "Not allowed"));
		}
	}

	match &mut item {
		AsyncItem::Struct(item) => attrs.parse_attrs(&mut item.attrs)?,
		AsyncItem::Trait(item) => attrs.parse_attrs(&mut item.attrs)?,
		AsyncItem::Impl(imp) => attrs.parse_attrs(&mut imp.attrs)?,
		_ => ()
	}

	if attrs.language.is_some() {
		return language_impl(attrs, item);
	}

	let item = match item {
		AsyncItem::Fn(item) => Functions::Fn(item),
		AsyncItem::TraitFn(item) => Functions::TraitFn(item),
		AsyncItem::Trait(item) => Functions::Trait(item),
		AsyncItem::Impl(item) => Functions::Impl(item),
		AsyncItem::Struct(item) => return Err(Error::new_spanned(item, "Unexpected declaration"))
	};

	match attrs.async_kind.0 {
		AsyncKind::Implicit => (),
		AsyncKind::TraitFn => return async_impl(attrs, item),
		AsyncKind::Sync => return item.transform_all(Some(&sync_doc_fn), transform_sync, |_| true),
		_ => return transform_items(item, attrs)
	}

	match item {
		Functions::Trait(item) => async_trait(attrs, item),
		Functions::Impl(ref imp) if imp.trait_.is_some() => async_impl(attrs, item),
		Functions::Fn(_) | Functions::Impl(_) => transform_items(item, attrs),
		Functions::TraitFn(func) => {
			let message = "Trait functions must specify `#[asynchronous(traitfn)]`";

			Err(Error::new_spanned(func, message))
		}
	}
}
