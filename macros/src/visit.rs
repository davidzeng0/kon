use super::*;

pub const SELF_IDENT: &str = "this";

pub struct ReplaceSelf;

impl VisitMut for ReplaceSelf {
	fn visit_item_mut(&mut self, _: &mut Item) {}

	fn visit_fn_arg_mut(&mut self, arg: &mut FnArg) {
		let FnArg::Receiver(rec) = arg else { return };

		let (attrs, mut mutability, ty) = (&rec.attrs, rec.mutability, &rec.ty);
		let ident = Ident::new(SELF_IDENT, Span::mixed_site());

		if rec.reference.is_some() {
			mutability = None;
		}

		*arg = parse_quote! {
			#(#attrs)*
			#mutability #ident: #ty
		}
	}

	fn visit_ident_mut(&mut self, ident: &mut Ident) {
		if ident == "self" {
			*ident = Ident::new(SELF_IDENT, Span::mixed_site());
		}
	}

	fn visit_macro_mut(&mut self, mac: &mut Macro) {
		visit_macro_body(self, mac);
	}
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Annotations {
	#[default]
	Counter,
	Uniform
}

pub struct AddLifetime {
	pub base: Lifetime,
	pub annotations: Annotations,
	pub explicit_lifetimes: Vec<Lifetime>,
	pub added_lifetimes: Vec<Lifetime>
}

impl AddLifetime {
	pub const fn new(base: Lifetime, annotations: Annotations) -> Self {
		Self {
			base,
			annotations,
			explicit_lifetimes: Vec::new(),
			added_lifetimes: Vec::new()
		}
	}

	fn next_lifetime(&mut self, span: Span) -> Lifetime {
		if self.annotations == Annotations::Uniform {
			return self.base.clone();
		}

		let lifetime = Lifetime::new(
			&format!("{}_{}", self.base, self.added_lifetimes.len() + 1),
			span
		);

		self.added_lifetimes.push(lifetime.clone());

		lifetime
	}
}

impl VisitMut for AddLifetime {
	fn visit_type_reference_mut(&mut self, reference: &mut TypeReference) {
		if let Some(lifetime) = &reference.lifetime {
			self.explicit_lifetimes.push(lifetime.clone());

			return;
		}

		if !matches!(reference.elem.as_ref(), Type::ImplTrait(_)) {
			visit_type_reference_mut(self, reference);
		}

		reference.lifetime = Some(self.next_lifetime(reference.span()));
	}

	fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
		if lifetime.ident == "_" {
			*lifetime = self.next_lifetime(lifetime.span());
		} else {
			self.explicit_lifetimes.push(lifetime.clone());
		}
	}

	fn visit_receiver_mut(&mut self, rec: &mut Receiver) {
		if let Some(reference) = &mut rec.reference {
			if let Some(lifetime) = &reference.1 {
				self.explicit_lifetimes.push(lifetime.clone());

				return;
			}

			let Type::Reference(ty_ref) = rec.ty.as_mut() else {
				unreachable!();
			};

			let lifetime = self.next_lifetime(reference.0.span());

			reference.1 = Some(lifetime.clone());
			ty_ref.lifetime = Some(lifetime);

			return;
		}

		visit_type_mut(self, rec.ty.as_mut());
	}

	fn visit_type_impl_trait_mut(&mut self, impl_trait: &mut TypeImplTrait) {
		impl_trait.bounds.push(TypeParamBound::Lifetime(
			self.next_lifetime(impl_trait.span())
		));
	}

	fn visit_signature_mut(&mut self, sig: &mut Signature) {
		for arg in &mut sig.inputs {
			self.visit_fn_arg_mut(arg);
		}
	}
}
