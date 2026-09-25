use crate::adapters::rust::model::{Context, CrateContext, SourceFile};
use crate::adapters::rust::paths::span_value;
use crate::adapters::rust::{declarations, extractor, implementations, imports};
use quote::ToTokens;
use syn::spanned::Spanned;

pub(crate) fn process_items(
    context: &mut Context,
    items: &[syn::Item],
    owner: &str,
    module: &str,
    source: &SourceFile,
    crate_context: &CrateContext,
) {
    for item in items {
        match item {
            syn::Item::Mod(value) => {
                module_item(context, value, owner, module, source, crate_context)
            }
            syn::Item::Struct(value) => {
                declarations::structure(context, value, owner, module, source)
            }
            syn::Item::Enum(value) => {
                declarations::enumeration(context, value, owner, module, source)
            }
            syn::Item::Type(value) => declarations::alias(context, value, owner, module, source),
            syn::Item::Const(value) => {
                declarations::value_type(context, &value.ident, &value.ty, owner, module, source)
            }
            syn::Item::Static(value) => {
                declarations::value_type(context, &value.ident, &value.ty, owner, module, source)
            }
            syn::Item::Trait(value) => crate::adapters::rust::declaration_special::trait_decl(
                context,
                value,
                owner,
                module,
                &crate_context.qn,
                source,
            ),
            syn::Item::Fn(value) => {
                declarations::function(context, value, owner, module, &crate_context.qn, source)
            }
            syn::Item::Impl(value) => {
                implementations::process(context, value, owner, module, &crate_context.qn, source)
            }
            syn::Item::Use(value) => {
                imports::record(context, value, owner, module, &crate_context.qn, source)
            }
            syn::Item::Macro(value) => crate::adapters::rust::declaration_special::macro_decl(
                context, value, owner, module, source,
            ),
            syn::Item::ExternCrate(value) => context.facts.add_unresolved(
                owner,
                "imports",
                &value.to_token_stream().to_string(),
                "external-target",
                span_value(value.span(), &source.relative),
            ),
            _ => {}
        }
    }
}

fn module_item(
    context: &mut Context,
    item: &syn::ItemMod,
    owner: &str,
    module: &str,
    source: &SourceFile,
    crate_context: &CrateContext,
) {
    let name = item.ident.to_string();
    let child_qn = format!("{module}::{name}");
    let id = declarations::add_node(
        context,
        "module",
        &child_qn,
        &name,
        source,
        item.span(),
        "module",
    );
    context.modules.insert(child_qn.clone(), id.clone());
    crate::adapters::rust::relationships::define_and_contain(
        context,
        owner,
        &id,
        item.span(),
        &source.relative,
    );
    if let Some((_, nested)) = &item.content {
        process_items(context, nested, &id, &child_qn, source, crate_context);
    } else if let Some(path) =
        crate::adapters::rust::paths::resolve_module_file(&context.sources, &source.path, &name)
    {
        extractor::process_file(context, &path, &id, &child_qn, crate_context);
    } else {
        context.facts.add_unresolved(
            owner,
            "contains",
            &format!("mod {name}"),
            "missing-target",
            span_value(item.span(), &source.relative),
        );
    }
}
