use super::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClosureType {
	None,
	Default,
	Trait
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[strings(defaults, lowercase)]
pub enum AsyncKind {
	#[omit]
	Implicit,
	TraitFn,
	TraitExt,
	Task,
	Sync
}

impl AsyncKind {
	#[must_use]
	pub const fn closure_type(self) -> ClosureType {
		match self {
			Self::Implicit => ClosureType::Default,
			Self::TraitExt => ClosureType::Trait,
			_ => ClosureType::None
		}
	}
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[strings(defaults, snake)]
pub enum Lang {
	GetContext,
	TaskWrap,
	AsyncClosure,
	Task
}

struct Ident(proc_macro2::Ident);

impl Parse for Ident {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		input
			.step(|cursor| {
				cursor
					.ident()
					.ok_or_else(|| cursor.error("Expected an identifier"))
			})
			.map(Self)
	}
}

impl ToTokens for Ident {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.0.to_tokens(tokens);
	}
}

#[derive(Clone, Copy)]
pub struct ImplGen {
	pub span: Span,
	pub impl_ref: Option<Span>,
	pub impl_mut: Option<Span>,
	pub impl_box: Option<Span>
}

impl Parse for ImplGen {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let options = Punctuated::<Ident, Token![,]>::parse_terminated(input)?;

		let mut this = Self {
			span: options.span(),
			impl_ref: None,
			impl_mut: None,
			impl_box: None
		};

		for option in options {
			let span = match option.0.to_string().as_ref() {
				"ref" => Some(&mut this.impl_ref),
				"mut" => Some(&mut this.impl_mut),
				"box" => Some(&mut this.impl_box),
				_ => None
			};

			if let Some(opt @ None) = span {
				*opt = Some(option.span());

				continue;
			}

			let message = if span.is_some() {
				"Duplicate option"
			} else {
				"Unknown option"
			};

			return Err(Error::new_spanned(option, message));
		}

		Ok(this)
	}
}

pub fn get_lang(attrs: &mut Vec<Attribute>) -> Result<Option<(Lang, Span)>> {
	let Some(attr) = attrs.remove_name_value("lang") else {
		return Ok(None);
	};

	let lang = (attr.value.parse_lit_str("Unknown lang item")?, attr.span());

	Ok(Some(lang))
}

#[derive(Clone, Copy)]
pub struct AttributeArgs {
	pub async_kind: (AsyncKind, Span),
	pub language: Option<(Lang, Span)>,
	pub impl_gen: Option<ImplGen>
}

impl AttributeArgs {
	pub const fn new(async_kind: AsyncKind, span: Span) -> Self {
		Self {
			async_kind: (async_kind, span),
			language: None,
			impl_gen: None
		}
	}

	pub fn parse(input: ParseStream<'_>) -> Result<Self> {
		let mut this = Self::new(AsyncKind::Implicit, input.span());

		while !input.is_empty() {
			let option = input.parse::<Ident>()?;
			let name = option.0.to_string();

			if name == "impl" {
				let content;

				parenthesized!(content in input);

				this.impl_gen = Some(ImplGen::parse(&content)?);
			} else {
				let kind = name
					.parse()
					.map_err(|()| Error::new_spanned(&option, "Unknown option"))?;

				if this.async_kind.0 != AsyncKind::Implicit {
					let message = "Duplicate async type modifier";

					return Err(Error::new_spanned(option, message));
				}

				this.async_kind = (kind, option.span());
			}
		}

		Ok(this)
	}

	pub fn parse_attrs(&mut self, attrs: &mut Vec<Attribute>) -> Result<()> {
		self.language = get_lang(attrs)?;

		Ok(())
	}
}
