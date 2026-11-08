use syn::{
    spanned::Spanned,
    visit::{self, Visit},
};

#[derive(Default)]
pub(crate) struct Checker {
    pub(crate) problems: Vec<String>,
}

impl Checker {
    fn function(&mut self, name: &syn::Ident, block: &syn::Block) {
        let lines = block.span().end().line - name.span().start().line + 1;
        if lines > 80 {
            self.problems
                .push(format!("function {name} has {lines} lines (limit 80)"));
        }
    }

    fn fields(&mut self, name: &syn::Ident, fields: &syn::Fields) {
        if fields.len() > 30 {
            self.problems.push(format!("{name} exceeds 30 fields"));
        }
    }
}

impl<'ast> Visit<'ast> for Checker {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.function(&node.sig.ident, &node.block);
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.function(&node.sig.ident, &node.block);
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if let Some(block) = &node.default {
            self.function(&node.sig.ident, block);
        }
        visit::visit_trait_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        self.fields(&node.ident, &node.fields);
        visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        if node.variants.len() > 30 {
            self.problems
                .push(format!("{} exceeds 30 variants", node.ident));
        }
        for variant in &node.variants {
            self.fields(&variant.ident, &variant.fields);
        }
        visit::visit_item_enum(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, syn::BinOp::AddAssign(_))
            && let syn::Expr::Lit(value) = &*node.right
            && let syn::Lit::Int(value) = &value.lit
            && value.base10_digits() == "1"
        {
            self.problems
                .push("increment by one must use wrapping_add(1)".into());
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_item_static(&mut self, node: &'ast syn::ItemStatic) {
        if matches!(node.mutability, syn::StaticMutability::Mut(_)) {
            self.problems.push(format!("mutable static {}", node.ident));
        }
        visit::visit_item_static(self, node);
    }
}

#[cfg(test)]
mod tests;
