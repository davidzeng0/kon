use super::*;

#[derive(Default)]
struct Attributes {
	no_display: Option<Span>,
	no_debug: Option<Span>
}

impl Attributes {
	fn parse(tokens: TokenStream) -> Result<Self> {
		let mut this = Self::default();
		let bounds = Punctuated::<TraitBound, Token![+]>::parse_terminated.parse2(tokens)?;

		for bound in bounds {
			if let Some(lt) = bound.lifetimes {
				return Err(Error::new_spanned(lt, "Not allowed"));
			}

			if !matches!(bound.modifier, TraitBoundModifier::Maybe(_)) {
				let msg = "Expected leading `?`";

				return Err(Error::new_spanned(bound, msg));
			}

			let msg = "Expected `Debug` or `Display`";
			let Some(ident) = &bound.path.get_ident() else {
				return Err(Error::new_spanned(bound.path, msg));
			};

			let option = match ident.to_string().as_ref() {
				"Display" => &mut this.no_display,
				"Debug" => &mut this.no_debug,
				_ => return Err(Error::new_spanned(bound.path, msg))
			};

			if option.is_some() {
				return Err(Error::new_spanned(bound.path, "Duplicate bound"));
			}

			*option = Some(ident.span());
		}

		Ok(this)
	}
}

#[derive(Default)]
struct VariantAttrs {
	from: Option<Attribute>,
	source: Option<(Attribute, usize)>
}

impl VariantAttrs {
	fn get_attrs(&mut self, field: &mut Field, index: usize) {
		if self.from.is_none() {
			self.from = field.attrs.remove_path("from");
		}

		if self.source.is_none() {
			let attr = field.attrs.remove_path("source");

			self.source = attr.map(|source| (source, index));
		}
	}
}

struct VariantFields {
	attributes: VariantAttrs,
	members: Vec<(Member, Type)>,
	named: bool
}

#[allow(clippy::missing_panics_doc)]
impl VariantFields {
	fn new(fields: &mut Fields) -> Self {
		let mut attributes = VariantAttrs::default();
		let mut members = Vec::new();

		match fields {
			Fields::Named(fields) => {
				for (i, field) in fields.named.iter_mut().enumerate() {
					let ident = field.ident.clone().unwrap();

					attributes.get_attrs(field, i);
					members.push((Member::Named(ident), field.ty.clone()));
				}
			}

			Fields::Unnamed(fields) => {
				for (i, field) in fields.unnamed.iter_mut().enumerate() {
					#[allow(clippy::cast_possible_truncation)]
					let index = Index { index: i as u32, span: field.ty.span() };

					attributes.get_attrs(field, i);
					members.push((Member::Unnamed(index), field.ty.clone()));
				}
			}

			Fields::Unit => ()
		}

		Self {
			attributes,
			members,
			named: !matches!(fields, Fields::Unnamed(_))
		}
	}
}

fn member_as_ident(member: &Member) -> Ident {
	match member {
		Member::Named(ident) => ident.clone(),
		Member::Unnamed(index) => format_ident!("f{}", index.index)
	}
}

struct Variant {
	display: Option<(TokenStream, bool)>,
	debug: Option<(TokenStream, bool)>,
	kind: Option<Expr>,
	ident: Ident,
	fields: VariantFields
}

fn maybe_transparent(attr: MetaList) -> (TokenStream, bool) {
	let parsed = parse2::<Ident>(attr.tokens.clone());
	let transparent = matches!(parsed, Ok(ident) if ident == "transparent");

	(attr.tokens, transparent)
}

impl Variant {
	fn new(attrs: &mut Vec<Attribute>, ident: &Ident, fields: &mut Fields) -> Result<Self> {
		let fmt = attrs.remove_list("fmt").map(maybe_transparent);

		let (display, debug) = if fmt.is_none() {
			(
				attrs.remove_list("display").map(maybe_transparent),
				attrs.remove_list("debug").map(maybe_transparent)
			)
		} else {
			(fmt.clone(), fmt)
		};

		let this = Self {
			display,
			debug,
			kind: attrs.remove_name_value("kind").map(|attr| attr.value),
			ident: ident.clone(),
			fields: VariantFields::new(fields)
		};

		if this.fields.members.len() == 1 {
			return Ok(this);
		}

		if let Some((display, true)) = &this.display {
			let msg = "#[display(transparent)] requires exactly one field";

			return Err(Error::new_spanned(display, msg));
		}

		if let Some((debug, true)) = &this.debug {
			let msg = "#[debug(transparent)] requires exactly one field";

			return Err(Error::new_spanned(debug, msg));
		}

		if let Some(from) = &this.fields.attributes.from {
			let msg = "#[from] requires exactly one field";

			return Err(Error::new_spanned(from, msg));
		}

		Ok(this)
	}

	fn matcher(&self) -> TokenStream {
		let ident = &self.ident;
		let mut fields = Punctuated::<Ident, Token![,]>::new();

		for (member, _) in &self.fields.members {
			fields.push(member_as_ident(member));
		}

		if self.fields.named {
			quote! { #ident { #fields } }
		} else {
			quote! { #ident ( #fields ) }
		}
	}

	fn display(&self, base: &Option<TokenStream>, fmt: &Ident) -> Option<TokenStream> {
		let (display, is_transparent) = self.display.as_ref()?;
		let write = if *is_transparent {
			let field = member_as_ident(&self.fields.members[0].0);

			quote! { ::std::fmt::Display::fmt(#field, #fmt) }
		} else {
			quote! { ::std::write!(#fmt, #display) }
		};

		let matcher = self.matcher();

		Some(quote! { #base #matcher => #write })
	}

	fn debug(&self, base: &Option<TokenStream>, fmt: &Ident) -> TokenStream {
		let matcher = self.matcher();

		let is_transparent = match self.debug {
			Some((_, tr)) => tr,
			None => matches!(self.display, Some((_, true)))
		};

		let write = if is_transparent {
			let field = member_as_ident(&self.fields.members[0].0);

			quote! { ::std::fmt::Debug::fmt(#field, #fmt) }
		} else if let Some((debug, _)) = self.debug.as_ref() {
			quote! { ::std::write!(#fmt, #debug) }
		} else {
			let mut debug = Punctuated::<Expr, Token![.]>::new();
			let ident = self.ident.to_string();

			debug.push(parse_quote! { #fmt });

			if self.fields.members.is_empty() {
				debug.push(parse_quote! { write_str(#ident) });
			} else if self.fields.named {
				debug.push(parse_quote! { debug_struct(#ident) });
			} else {
				debug.push(parse_quote! { debug_tuple(#ident) });
			}

			for (member, _) in &self.fields.members {
				debug.push(match member {
					Member::Named(ident) => {
						let name = ident.to_string();

						parse_quote! { field(#name, #ident) }
					}

					Member::Unnamed(_) => {
						let ident = member_as_ident(member);

						parse_quote! { field(#ident) }
					}
				});
			}

			if !self.fields.members.is_empty() {
				debug.push(parse_quote! { finish() });
			}

			quote! { #debug }
		};

		quote! { #base #matcher => #write }
	}

	fn kind(&self, base: &Option<TokenStream>) -> Option<TokenStream> {
		let kind = self.kind.as_ref()?;
		let matcher = self.matcher();

		Some(quote! { #base #matcher => #kind })
	}

	fn from(&self, base: &Option<TokenStream>) -> Option<(TokenStream, &Type)> {
		let _ = self.fields.attributes.from.as_ref()?;

		let ident = &self.ident;
		let (member, ty) = &self.fields.members[0];

		Some((quote! { #base #ident { #member: value } }, ty))
	}

	fn source(&self, base: &Option<TokenStream>) -> Option<TokenStream> {
		let (_, index) = self.fields.attributes.source.as_ref()?;

		let matcher = self.matcher();
		let member = member_as_ident(&self.fields.members[*index].0);

		Some(quote! { #base #matcher => Some(#member) })
	}
}

enum Repr {
	Struct(Variant),
	Enum(Vec<Variant>)
}

fn add_bounds(
	generics: &Generics, where_clause: Option<&WhereClause>, bounds: &[TypeParamBound]
) -> WhereClause {
	let mut clause = where_clause.cloned().unwrap_or_else(WhereClause::default);

	for ty in generics.type_params() {
		let ty = &ty.ident;

		clause.predicates.push(parse_quote! { #ty: #(#bounds)+* });
	}

	clause
}

struct Input {
	attrs: Attributes,
	input: DeriveInput,
	repr: Repr
}

impl Input {
	fn parse_enum(item: &mut DataEnum) -> Result<Vec<Variant>> {
		let mut variants = Vec::new();

		for variant in &mut item.variants {
			variants.push(Variant::new(
				&mut variant.attrs,
				&variant.ident,
				&mut variant.fields
			)?);
		}

		Ok(variants)
	}

	fn parse(item: TokenStream, attrs: Attributes) -> Result<Self> {
		let mut input: DeriveInput = parse2(item)?;

		if attrs.no_debug.is_none() {
			input.attrs.push(parse_quote! {
				#[derive(::std::fmt::Debug)]
			});
		}

		input.attrs.push(parse_quote! {
			#[allow(missing_copy_implementations)]
		});

		let repr = match &mut input.data {
			Data::Struct(data) => Repr::Struct(Variant::new(
				&mut input.attrs,
				&input.ident,
				&mut data.fields
			)?),

			Data::Enum(data) => Repr::Enum(Self::parse_enum(data)?),
			Data::Union(_) => return Err(Error::new_spanned(input, "Unions are not supported"))
		};

		Ok(Self { attrs, input, repr })
	}

	fn expand(&self) -> Result<TokenStream> {
		let orig = &self.input;
		let name = &self.input.ident;
		let (impl_generics, type_generics, where_clause) = self.input.generics.split_for_impl();

		let fmt = Ident::new("fmt", Span::mixed_site());
		let base = Some(quote! { Self:: });

		let mut displays = Punctuated::<_, Token![,]>::new();
		let mut debugs = Punctuated::<_, Token![,]>::new();
		let mut kinds = Vec::new();
		let mut sources = Vec::new();
		let mut froms = Vec::new();

		let eq = match &self.repr {
			Repr::Struct(variant) => {
				if let Some(display) = variant.display(&None, &fmt) {
					displays.push(display);
				}

				if let Some(kind) = variant.kind(&None) {
					kinds.push(kind);
				}

				if let Some(from) = variant.from(&None) {
					froms.push(from);
				}

				if let Some(source) = variant.source(&None) {
					sources.push(source);
				}

				if self.attrs.no_debug.is_some() {
					debugs.push(variant.debug(&None, &fmt));
				}

				quote! { true }
			}

			Repr::Enum(variants) => {
				let has_display = variants.iter().any(|variant| variant.display.is_some());

				for variant in variants {
					if has_display {
						let Some(display) = variant.display(&base, &fmt) else {
							let msg = "Missing #[display(..)] attribute";

							return Err(Error::new_spanned(&variant.ident, msg));
						};

						displays.push(display);
					}

					if let Some(kind) = variant.kind(&base) {
						kinds.push(kind);
					}

					if let Some(from) = variant.from(&base) {
						froms.push(from);
					}

					if let Some(source) = variant.source(&base) {
						sources.push(source);
					}

					if self.attrs.no_debug.is_some() {
						debugs.push(variant.debug(&base, &fmt));
					}
				}

				quote! {
					::std::mem::discriminant(self) == ::std::mem::discriminant(other)
				}
			}
		};

		let mut fmts = Vec::new();

		if !displays.is_empty() {
			let clause;
			let where_clause = if self.attrs.no_display.is_some() {
				where_clause
			} else {
				clause = add_bounds(
					&self.input.generics,
					where_clause,
					&[parse_quote! { ::std::fmt::Display }]
				);

				Some(&clause)
			};

			let gen_fmt = |trait_name, fmts| {
				quote! {
					impl #impl_generics ::std::fmt::#trait_name for #name #type_generics #where_clause {
						fn fmt(&self, #fmt: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
							#[allow(unused_variables)]
							match self {
								#fmts
							}
						}
					}
				}
			};

			fmts.push(gen_fmt(quote! { Display }, &displays));

			if self.attrs.no_debug.is_some() {
				fmts.push(gen_fmt(quote! { Debug }, &debugs));
			}
		}

		let mut from_impls = Vec::new();

		for (body, ty) in froms {
			from_impls.push(quote_spanned! { ty.span() =>
				impl #impl_generics ::std::convert::From<#ty> for #name #type_generics #where_clause {
					fn from(value: #ty) -> Self {
						#body
					}
				}
			});
		}

		Ok(quote! {
			#orig
			#(#fmts)*

			#(#from_impls)*

			impl #impl_generics ::std::cmp::PartialEq for #name #type_generics #where_clause {
				fn eq(&self, other: &Self) -> bool {
					#eq
				}
			}

			impl #impl_generics ::std::error::Error for #name #type_generics #where_clause
			where
				Self: ::std::fmt::Debug + ::std::fmt::Display
			{
				fn source(&self) -> ::std::option::Option<&(dyn ::std::error::Error + 'static)> {
					#[allow(unused_variables)]
					match self {
						#(#sources,)*
						_ => None
					}
				}
			}

			impl #impl_generics ::xx_core::error::internal::ErrorImpl for #name #type_generics #where_clause
			where
				Self: ::std::error::Error + ::std::marker::Send
					+ ::std::marker::Sync + 'static
			{
				fn kind(&self) -> ::xx_core::error::ErrorKind {
					#[allow(unused_variables)]
					match self {
						#(#kinds,)*
						_ => ::xx_core::error::ErrorKind::Other
					}
				}
			}
		})
	}
}

pub fn errors(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
	let attrs = Attributes::parse(attr)?;
	let input = Input::parse(item, attrs)?;

	input.expand()
}
