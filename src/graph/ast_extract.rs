use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RustAstSummary {
    pub modules: Vec<String>,
    pub functions: Vec<String>,
    pub structs: Vec<String>,
    pub enums: Vec<String>,
    pub use_imports: Vec<String>,
    pub test_functions: Vec<String>,
}

pub fn extract_rust_ast(source: &str) -> Result<RustAstSummary, String> {
    let file =
        syn::parse_file(source).map_err(|error| format!("failed to parse Rust AST: {error}"))?;
    let mut summary = RustAstSummary::default();
    collect_items(&file.items, &mut summary);
    summary.sort_compact();
    Ok(summary)
}

fn collect_items(items: &[syn::Item], summary: &mut RustAstSummary) {
    for item in items {
        match item {
            syn::Item::Mod(module) => {
                summary.modules.push(module.ident.to_string());
                if let Some((_, nested)) = &module.content {
                    collect_items(nested, summary);
                }
            }
            syn::Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if function.attrs.iter().any(is_test_attr) {
                    summary.test_functions.push(name.clone());
                }
                summary.functions.push(name);
            }
            syn::Item::Struct(item_struct) => summary.structs.push(item_struct.ident.to_string()),
            syn::Item::Enum(item_enum) => summary.enums.push(item_enum.ident.to_string()),
            syn::Item::Use(item_use) => {
                summary.use_imports.push(use_tree_to_string(&item_use.tree))
            }
            _ => {}
        }
    }
}

fn is_test_attr(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("test")
}

fn use_tree_to_string(tree: &syn::UseTree) -> String {
    match tree {
        syn::UseTree::Path(path) => format!("{}::{}", path.ident, use_tree_to_string(&path.tree)),
        syn::UseTree::Name(name) => name.ident.to_string(),
        syn::UseTree::Rename(rename) => format!("{} as {}", rename.ident, rename.rename),
        syn::UseTree::Glob(_) => "*".to_string(),
        syn::UseTree::Group(group) => group
            .items
            .iter()
            .map(use_tree_to_string)
            .collect::<Vec<_>>()
            .join(","),
    }
}

impl RustAstSummary {
    fn sort_compact(&mut self) {
        self.modules.sort();
        self.modules.dedup();
        self.functions.sort();
        self.functions.dedup();
        self.structs.sort();
        self.structs.dedup();
        self.enums.sort();
        self.enums.dedup();
        self.use_imports.sort();
        self.use_imports.dedup();
        self.test_functions.sort();
        self.test_functions.dedup();
    }
}
