use crate::adapters::rust::dependencies::dependency_attributes;
use crate::adapters::rust::model::{Context, PendingImport};
use crate::adapters::rust::paths::{span_start, span_value};
use crate::adapters::rust::resolve;
use crate::adapters::rust::syntax::normalized_tokens;
use quote::ToTokens;
use syn::UseTree;
use syn::spanned::Spanned;

#[derive(Clone)]
struct Binding {
    path: String,
    alias: Option<String>,
    glob: bool,
}

pub(crate) fn record(
    context: &mut Context,
    item: &syn::ItemUse,
    owner: &str,
    module: &str,
    crate_qn: &str,
    source: &crate::adapters::rust::model::SourceFile,
) {
    let expression = item.to_token_stream().to_string();
    let start = span_start(item.span());
    let qn = format!(
        "{module}::use:{}@{}:{}",
        normalized_tokens(&item.tree),
        start.0,
        start.1
    );
    let id = context.facts.add_node(
        "rust",
        "import",
        &qn,
        &expression,
        &source.relative,
        &qn,
        None,
        span_value(item.span(), &source.relative),
        Default::default(),
    );
    crate::adapters::rust::relationships::define_and_contain(
        context,
        owner,
        &id,
        item.span(),
        &source.relative,
    );
    context.pending_imports.push(PendingImport {
        owner_id: owner.into(),
        module_qn: module.into(),
        crate_qn: crate_qn.into(),
        item: item.clone(),
        expression,
        span: span_value(item.span(), &source.relative),
    });
}

pub(crate) fn resolve_all(context: &mut Context) {
    let pending = context.pending_imports.clone();
    for _ in 0..4 {
        let mut changed = false;
        for item in &pending {
            let mut bindings = Vec::new();
            flatten(&item.item.tree, &mut Vec::new(), &mut bindings);
            for binding in bindings {
                changed |= install(context, item, &binding, false);
            }
        }
        if !changed {
            break;
        }
    }
    for item in pending {
        let mut bindings = Vec::new();
        flatten(&item.item.tree, &mut Vec::new(), &mut bindings);
        for binding in bindings {
            install(context, &item, &binding, true);
        }
    }
}

fn install(context: &mut Context, item: &PendingImport, binding: &Binding, emit: bool) -> bool {
    let builtin = builtin_root(&binding.path);
    let external = !builtin && external_root(context, &binding.path, &item.crate_qn);
    if builtin || external {
        let alias = binding
            .alias
            .clone()
            .unwrap_or_else(|| binding.path.split("::").last().unwrap_or_default().into());
        let scope = context.imports.entry(item.module_qn.clone()).or_default();
        return if builtin {
            scope.builtin_aliases.insert(alias)
        } else {
            scope.external_aliases.insert(alias)
        };
    }
    let targets = resolve::resolve_any_qns(context, &binding.path, &item.module_qn, &item.crate_qn);
    let mut changed = false;
    if binding.glob {
        for target in &targets {
            if context.modules.contains_key(target) {
                changed |= context
                    .imports
                    .entry(item.module_qn.clone())
                    .or_default()
                    .glob_modules
                    .insert(target.clone());
            }
        }
    } else {
        let alias = binding
            .alias
            .clone()
            .unwrap_or_else(|| binding.path.split("::").last().unwrap_or_default().into());
        let scope = context.imports.entry(item.module_qn.clone()).or_default();
        let values = scope.bindings.entry(alias).or_default();
        for target in &targets {
            if !values.contains(target) {
                values.push(target.clone());
                changed = true;
            }
        }
        values.sort();
        values.dedup();
    }
    if emit {
        if targets.is_empty() {
            context.facts.add_unresolved(
                &item.owner_id,
                "imports",
                &item.expression,
                if binding.path.starts_with("crate::")
                    || binding.path.starts_with("self::")
                    || binding.path.starts_with("super::")
                {
                    "dynamic-target"
                } else {
                    "external-target"
                },
                item.span.clone(),
            );
        } else {
            for qn in targets {
                if let Some(id) = any_id(context, &qn) {
                    context
                        .facts
                        .add_edge(&item.owner_id, &id, "imports", item.span.clone());
                    if let (Some(source_module), Some(target_module)) = (
                        context.modules.get(&item.module_qn),
                        context.modules.get(&qn),
                    ) {
                        context.facts.add_edge_with_attributes(
                            source_module,
                            target_module,
                            "depends-on",
                            item.span.clone(),
                            dependency_attributes("local", &item.expression, "", false, true),
                        );
                    }
                }
            }
        }
    }
    changed
}

fn flatten(tree: &UseTree, prefix: &mut Vec<String>, output: &mut Vec<Binding>) {
    match tree {
        UseTree::Path(value) => {
            prefix.push(value.ident.to_string());
            flatten(&value.tree, prefix, output);
            prefix.pop();
        }
        UseTree::Name(value) if value.ident == "self" => output.push(Binding {
            path: prefix.join("::"),
            alias: prefix.last().cloned(),
            glob: false,
        }),
        UseTree::Name(value) => {
            let mut path = prefix.clone();
            path.push(value.ident.to_string());
            output.push(Binding {
                path: path.join("::"),
                alias: None,
                glob: false,
            });
        }
        UseTree::Rename(value) => {
            let mut path = prefix.clone();
            path.push(value.ident.to_string());
            output.push(Binding {
                path: path.join("::"),
                alias: Some(value.rename.to_string()),
                glob: false,
            });
        }
        UseTree::Glob(_) => output.push(Binding {
            path: prefix.join("::"),
            alias: None,
            glob: true,
        }),
        UseTree::Group(value) => {
            for item in &value.items {
                flatten(item, prefix, output);
            }
        }
    }
}

fn any_id(context: &Context, qn: &str) -> Option<String> {
    context
        .symbols
        .get(qn)
        .or_else(|| context.modules.get(qn))
        .or_else(|| context.types.get(qn))
        .or_else(|| context.traits.get(qn))
        .or_else(|| context.macros.get(qn))
        .or_else(|| context.constructors.get(qn))
        .cloned()
}

fn builtin_root(path: &str) -> bool {
    matches!(
        path.trim_start_matches("::")
            .split("::")
            .next()
            .unwrap_or_default(),
        "std" | "core" | "alloc" | "proc_macro" | "test"
    )
}

fn external_root(context: &Context, path: &str, crate_qn: &str) -> bool {
    let root = path
        .trim_start_matches("::")
        .split("::")
        .next()
        .unwrap_or_default();
    context
        .crates
        .iter()
        .find(|item| item.qn == crate_qn)
        .is_some_and(|item| item.external_crates.contains(root))
}
