#![allow(dead_code)]

use std::collections::VecDeque;

use super::*;

enum Mode<'a> {
	Select(&'a [Branch]),
	Join
}

fn vars(items: usize, prefix: &str) -> Vec<Ident> {
	(0..items)
		.map(|index| format_ident!("{}{}", prefix, index))
		.collect()
}

#[allow(clippy::missing_panics_doc)]
fn branch(env: &Expr, tasks: &[&Expr], mode: Mode<'_>) -> TokenStream {
	let fut_vars = vars(tasks.len(), "fut");

	let (tasks_decl, futs_decl) = {
		let task_vars = vars(tasks.len(), "task");

		(
			quote! {
				#(let #task_vars = #tasks;)*
				let __spawn_env = #env;
			},
			quote! {
				#(let #fut_vars = spawn_task_with_env(__spawn_env, #task_vars);)*
			}
		)
	};

	let mut futures = VecDeque::<(Expr, TokenStream, TokenStream)>::new();

	let variants: Vec<_> = vars(tasks.len(), "N");
	let results: Vec<_> = vars(tasks.len(), "result");

	let (pre, cancel, post, pre_map, post_map) = match mode {
		Mode::Select(branches) => {
			for (fut, variant) in fut_vars.iter().zip(variants.iter()) {
				futures.push_back((
					parse_quote! { #fut },
					parse_quote! {
						select_result = XXInternalSelectResult::#variant(
							runtime::join(result)
						);
					},
					parse_quote! { let _ = other.map(runtime::join); }
				));
			}

			let mut matches = Vec::new();
			let mut variants_iter = variants.iter();

			for branch in branches {
				let Branch { pat, guard, handler, comma, .. } = branch;
				let guard = guard
					.as_ref()
					.map(|(if_token, expr)| quote! { #if_token #expr });

				if branch.task.is_some() {
					let variant = variants_iter.next().unwrap();

					matches.push(quote! {
						XXInternalSelectResult::#variant(#pat) #guard => #handler #comma
					});
				} else {
					matches.push(quote! {
						#pat #guard => #handler #comma
					});
				}
			}

			(
				quote! {
					enum XXInternalSelectResult<#(#variants),*> {
						#(#variants(#variants)),*
					}
				},
				quote! { |_| true },
				quote! {
					match result {
						#(#matches)*
					}
				},
				quote! { let select_result; },
				quote! { select_result }
			)
		}

		Mode::Join => {
			for (fut, res) in fut_vars.iter().zip(results.iter()) {
				futures.push_back((
					parse_quote! { #fut },
					parse_quote! { #res = runtime::join(result.unwrap_unchecked()); },
					parse_quote! { #res = runtime::join(other.unwrap_unchecked()); }
				));
			}

			(
				quote! { #(let #results;)* },
				quote! { |result| !::std::matches!(result, Ok(Ok(_))) },
				quote! {},
				quote! {},
				quote! { (#(#results),*) }
			)
		}
	};

	let mut branch_vars = Vec::<TokenStream>::new();
	let mut branch_number = 0usize;

	while futures.len() >= 2 {
		let len = futures.len();

		for _ in 0..len / 2 {
			let branch = format_ident!("branch{}", branch_number);

			branch_number += 1;

			let (task_a, first_a, second_a) = futures.pop_front().unwrap();
			let (task_b, first_b, second_b) = futures.pop_front().unwrap();

			branch_vars.push(parse_quote! {
				let mut #branch = Branch::new(
					#task_a,
					#task_b,
					(#cancel, #cancel)
				);

				let mut #branch = PinExt::pin_local(&mut #branch);
			});

			let (first, second) = match mode {
				Mode::Select(_) => (
					quote! {
						match Select::from_branch(result) {
							Select::First(result, other) => {
								#first_a
								#second_b
							}

							Select::Second(result, other) => {
								#first_b
								#second_a
							}
						}
					},
					quote! {
						if let Some(
							BranchOutput(_, other0, other1)
						) = other {
							let other = other0.map(runtime::join);

							#second_a

							let other = other1.map(runtime::join);

							#second_b
						}
					}
				),

				Mode::Join => {
					let body = quote! {
						let BranchOutput(_, result, other) = result;
						let result = result.unwrap_unchecked();

						{ #first_a }
						{ #second_b }
					};

					(
						quote! {
							#body
						},
						quote! {
							let result = other;

							#body
						}
					)
				}
			};

			futures.push_back((
				parse_quote! { Branch::run(ptr!(&mut *#branch)) },
				first,
				second
			));
		}

		if len % 2 != 0 {
			let fut = futures.pop_front().unwrap();

			futures.push_back(fut);
		}
	}

	let (fut, map, _) = futures.pop_front().unwrap();

	let mut task = parse_quote! {
		async {
			#tasks_decl
			#pre

			let result = {
				use ::xx_core::{
					runtime,
					pointer::{ptr, PinExt},
					coroutines::{
						spawn_task_with_env, block_on,
						branch::{Branch, BranchOutput},
						Select, Join
					}
				};

				#futs_decl

				#(#branch_vars)*

				let result = block_on(#fut).await;

				#pre_map
				#map
				#post_map
			};

			#post
		}
	};

	TransformSync.visit_expr_mut(&mut task);

	task.to_token_stream()
}

#[derive(Clone)]
struct Branch {
	pat: Pat,
	task: Option<Expr>,
	guard: Option<(Token![if], Expr)>,
	handler: Expr,
	comma: Option<Token![,]>
}

struct Select {
	env: Expr,
	branches: Vec<Branch>
}

impl Select {
	fn expand(&self) -> Result<TokenStream> {
		let (env, branches) = (&self.env, &self.branches);

		let tasks: Vec<_> = branches
			.iter()
			.filter_map(|branch| branch.task.as_ref())
			.collect();
		if tasks.len() < 2 {
			return Err(Error::new(
				Span::call_site(),
				"Select takes a minimum of two branches"
			));
		}

		Ok(branch(env, &tasks, Mode::Select(branches)))
	}
}

impl Parse for Select {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let mut branches = Vec::new();
		let env = input.parse()?;

		input.parse::<Token![;]>()?;

		while !input.is_empty() {
			let pat = Pat::parse_multi(input)?;

			let task = if !input.peek(Token![=>]) {
				input.parse::<Token![=]>()?;

				Some(input.parse()?)
			} else {
				None
			};

			let guard = if let Some(if_token) = input.parse::<Option<Token![if]>>()? {
				Some((if_token, input.parse()?))
			} else {
				None
			};

			input.parse::<Token![=>]>()?;

			let handler: Expr = input.parse()?;

			branches.push(Branch { pat, task, guard, handler, comma: input.parse()? });
		}

		Ok(Self { env, branches })
	}
}

pub fn select(item: TokenStream) -> Result<TokenStream> {
	parse2::<Select>(item).and_then(|select| select.expand())
}

struct Join {
	env: Expr,
	tasks: Punctuated<Expr, Token![,]>
}

impl Join {
	fn expand(&self) -> Result<TokenStream> {
		let (env, tasks) = (&self.env, &self.tasks);

		if tasks.len() < 2 {
			return Err(Error::new(
				Span::call_site(),
				"Join takes a minimum of two branches"
			));
		}

		let tasks: Vec<_> = tasks.iter().collect();

		Ok(branch(env, &tasks, Mode::Join))
	}
}

impl Parse for Join {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let env = input.parse()?;

		input.parse::<Token![;]>()?;

		let tasks = Punctuated::<Expr, Token![,]>::parse_terminated(input)?;

		Ok(Self { env, tasks })
	}
}

pub fn join(_: TokenStream) -> Result<TokenStream> {
	// let join = match parse2::<Join>(item) {
	// 	Ok(join) => join,
	// 	Err(err) => return err.to_compile_error()
	// };

	// join.expand()
	Err(Error::new(
		Span::call_site(),
		"This macro currently does not work"
	))
}
