use super::*;

#[strings(defaults, lowercase)]
#[repr(u8)]
enum Unit {
	#[alt = "d"]
	#[alt = "days"]
	Day,

	#[alt = "h"]
	#[alt = "hr"]
	#[alt = "hrs"]
	#[alt = "hours"]
	Hour,

	#[alt = "m"]
	#[alt = "min"]
	#[alt = "mins"]
	#[alt = "minutes"]
	Minute,

	#[alt = "s"]
	#[alt = "sec"]
	#[alt = "secs"]
	#[alt = "seconds"]
	Second,

	#[alt = "ms"]
	#[alt = "millis"]
	#[alt = "millisec"]
	#[alt = "millisecs"]
	#[alt = "milliseconds"]
	Millisecond,

	#[alt = "us"]
	#[alt = "micros"]
	#[alt = "microsec"]
	#[alt = "microsecs"]
	#[alt = "microseconds"]
	Microsecond,

	#[alt = "ns"]
	#[alt = "nanos"]
	#[alt = "nanosec"]
	#[alt = "nanosecs"]
	#[alt = "nanoseconds"]
	Nanosecond
}

const NAMED_SCALES: &[(Unit, f64)] = &[
	(Unit::Day, 24.0),
	(Unit::Hour, 60.0),
	(Unit::Minute, 60.0),
	(Unit::Second, 1000.0),
	(Unit::Millisecond, 1000.0),
	(Unit::Microsecond, 1000.0),
	(Unit::Nanosecond, 1.0)
];

#[derive(Default)]
struct Duration {
	expr: Punctuated<Expr, Token![+]>,
	seps: Vec<TokenStream>,
	nanos: f64
}

impl Duration {
	fn push(&mut self, scale: f64, scalar: Expr) -> Result<()> {
		#[allow(clippy::cast_precision_loss)]
		if let Expr::Lit(ExprLit { lit: Lit::Int(lit), .. }) = scalar {
			self.nanos += lit.base10_parse::<u128>()? as f64 * scale;
		} else if let Expr::Lit(ExprLit { lit: Lit::Float(lit), .. }) = scalar {
			self.nanos += lit.base10_parse::<f64>()? * scale;
		} else {
			self.expr.push(parse_quote_spanned! { scalar.span() =>
				(#scalar) as f64 * #scale
			});
		}

		Ok(())
	}

	fn add_sep(&mut self, sep: Span) {
		let sep = format_ident!("Sep{}", self.seps.len(), span = sep);

		self.seps.push(quote! {
			enum #sep {}
		});
	}
}

impl ToTokens for Duration {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let (expr, seps, nanos) = (&self.expr, &self.seps, self.nanos);

		let nanos = if nanos != 0.0 {
			Some(if !expr.is_empty() {
				quote! { + #nanos }
			} else {
				quote! { #nanos }
			})
		} else {
			None
		};

		quote! {{
			const _: () = {
				#(#seps)*
			};

			#expr #nanos
		}}
		.to_tokens(tokens);
	}
}

fn parse_named_units(input: ParseStream<'_>) -> Result<TokenStream> {
	let mut parsed = Duration::default();

	loop {
		let scalar = input.parse::<Expr>()?;
		let unit_ident = input.parse::<Ident>()?;

		let unit: Unit = unit_ident
			.to_string()
			.parse()
			.map_err(|()| Error::new_spanned(&unit_ident, "Unknown unit"))?;
		let scale = NAMED_SCALES[(unit as usize)..]
			.iter()
			.fold(1.0, |acc, value| acc * value.1);
		parsed.push(scale, scalar)?;
		parsed.add_sep(unit_ident.span());

		if input.is_empty() {
			break;
		}
	}

	Ok(parsed.to_token_stream())
}

const UNNAMED_SCALES: &[f64] = &[24.0, 60.0, 60.0, 1_000_000_000.0];

fn parse_unnamed_units(input: ParseStream<'_>) -> Result<TokenStream> {
	let mut parsed = Duration::default();
	let mut scalars = Vec::new();

	loop {
		scalars.push(input.parse::<Expr>()?);

		if input.is_empty() {
			break;
		}

		let sep = if !input.peek(Token![::]) {
			input.parse::<Token![:]>()?.span()
		} else {
			input.parse::<Token![::]>()?.span()
		};

		parsed.add_sep(sep);
	}

	if scalars.len() > UNNAMED_SCALES.len() {
		let msg = "Too many separators";

		return Err(Error::new_spanned(&scalars[UNNAMED_SCALES.len()], msg));
	}

	for (index, scalar) in scalars.into_iter().rev().enumerate() {
		let index = UNNAMED_SCALES.len() - index - 1;
		let scale = UNNAMED_SCALES[index..]
			.iter()
			.fold(1.0, |acc, value| acc * value);
		parsed.push(scale, scalar)?;
	}

	Ok(parsed.to_token_stream())
}

fn parse_time_string(input: ParseStream<'_>) -> Result<TokenStream> {
	if input.peek2(Token![:]) || input.peek2(Token![::]) {
		parse_unnamed_units(input)
	} else {
		parse_named_units(input)
	}
}

fn parse_inverse(expr: Expr) -> Result<TokenStream> {
	let Expr::Binary(binary) = expr else {
		return Err(Error::new_spanned(expr, "Expected a ratio"));
	};

	let BinOp::Div(_) = binary.op else {
		return Err(Error::new_spanned(binary.op, "Expected a ratio"));
	};

	let (left, right) = (&binary.left, &binary.right);

	Ok(quote! {{
		let (left, right) = (#left, #right);

		left as u128 * 1_000_000_000 / right as u128
	}})
}

pub fn duration(item: TokenStream) -> Result<TokenStream> {
	let nanos = if let Ok(expr) = parse2::<Expr>(item.clone()) {
		parse_inverse(expr)?
	} else {
		parse_time_string.parse2(item)?
	};

	Ok(quote! {{
		#[allow(clippy::unnecessary_cast)]
		::std::time::Duration::from_nanos((#nanos) as u64)
	}})
}
