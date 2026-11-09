use syn::{
    spanned::Spanned,
    visit::{self, Visit},
};

#[derive(Default)]
pub(crate) struct Checker {
    pub(crate) problems: Vec<String>,
    pub(crate) test_file: bool,
    function_name: Option<String>,
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
        let previous = self.function_name.replace(node.sig.ident.to_string());
        visit::visit_item_fn(self, node);
        self.function_name = previous;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.function(&node.sig.ident, &node.block);
        let previous = self.function_name.replace(node.sig.ident.to_string());
        visit::visit_impl_item_fn(self, node);
        self.function_name = previous;
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

    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        if node.path().is_ident("path") {
            self.problems
                .push("module paths must follow the filesystem layout".into());
        }
        if node.path().is_ident("test") && !self.test_file {
            self.problems
                .push("unit tests belong in domain tests.rs files".into());
        }
        visit::visit_attribute(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if !self.test_file
            && node.path.segments.last().is_some_and(|segment| {
                matches!(
                    segment.ident.to_string().as_str(),
                    "println" | "eprintln" | "dbg"
                )
            })
        {
            self.problems.push("use tracing for diagnostics".into());
        }
        visit::visit_macro(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if !self.test_file
            && let syn::Expr::Path(function) = &*node.func
        {
            let parts: Vec<_> = function
                .path
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .collect();
            if parts.iter().any(|part| part == "fs")
                && self
                    .function_name
                    .as_ref()
                    .is_some_and(|name| name != "main" && !name.ends_with("_blocking"))
            {
                self.problems
                    .push("filesystem operations require a _blocking function".into());
            }
            if parts.iter().any(|part| part == "env")
                && parts
                    .last()
                    .is_some_and(|name| name == "var" || name == "var_os")
            {
                self.problems
                    .push("pass environment configuration through a central input".into());
            }
        }
        visit::visit_expr_call(self, node);
    }
}

#[cfg(test)]
mod tests;
