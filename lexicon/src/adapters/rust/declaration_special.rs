use crate::adapters::rust::declarations::{add_node, value_type};
use crate::adapters::rust::function_index::{self, Registration};
use crate::adapters::rust::model::{Context, FunctionBody, MethodInfo, SourceFile};
use crate::adapters::rust::paths::span_value;
use crate::adapters::rust::syntax::normalized_tokens;
use quote::ToTokens;
use syn::spanned::Spanned;

pub(crate) fn trait_decl(
    context: &mut Context,
    item: &syn::ItemTrait,
    owner: &str,
    module: &str,
    crate_qn: &str,
    source: &SourceFile,
) {
    let name = item.ident.to_string();
    let qn = format!("{module}::{name}");
    let id = add_node(context, "trait", &qn, &name, source, item.span(), "trait");
    context.symbols.insert(qn.clone(), id.clone());
    context.traits.insert(qn.clone(), id.clone());
    context.trait_qn_by_id.insert(id.clone(), qn.clone());
    crate::adapters::rust::relationships::define_and_contain(
        context,
        owner,
        &id,
        item.span(),
        &source.relative,
    );
    for trait_item in &item.items {
        if let syn::TraitItem::Fn(method) = trait_item {
            let method_name = method.sig.ident.to_string();
            let method_qn = format!("{qn}::{method_name}");
            let method_id = add_node(
                context,
                "method",
                &method_qn,
                &method_name,
                source,
                method.span(),
                "trait-method",
            );
            context.symbols.insert(method_qn.clone(), method_id.clone());
            context.trait_method_ids.insert(method_id.clone());
            context
                .trait_method_index
                .entry((id.clone(), method_name.clone()))
                .or_default()
                .push(method_id.clone());
            context
                .function_qn_by_id
                .insert(method_id.clone(), method_qn.clone());
            crate::adapters::rust::relationships::define_and_contain(
                context,
                &id,
                &method_id,
                method.span(),
                &source.relative,
            );
            context.methods.push(MethodInfo {
                id: method_id.clone(),
                self_type: "Self".into(),
                trait_path: Some(qn.clone()),
                name: method_name,
                module_qn: module.into(),
                crate_qn: crate_qn.into(),
            });
            if let Some(block) = &method.default {
                function_index::register(
                    context,
                    Registration {
                        id: method_id,
                        qn: method_qn,
                        module_qn: module,
                        crate_qn,
                        source_path: &source.relative,
                        signature: &method.sig,
                        body: FunctionBody::Block(block.clone()),
                        self_type: Some("Self".into()),
                        trait_path: Some(qn.clone()),
                    },
                );
            }
        }
    }
}

pub(crate) fn macro_decl(
    context: &mut Context,
    item: &syn::ItemMacro,
    owner: &str,
    module: &str,
    source: &SourceFile,
) {
    let Some(ident) = &item.ident else {
        if normalized_tokens(&item.mac.path) == "thread_local"
            && let Ok(file) = syn::parse2::<syn::File>(item.mac.tokens.clone())
        {
            for nested in file.items {
                if let syn::Item::Static(value) = nested {
                    value_type(context, &value.ident, &value.ty, owner, module, source);
                }
            }
        }
        context.facts.add_unresolved(
            owner,
            "defines",
            &item.to_token_stream().to_string(),
            "generated-target",
            span_value(item.span(), &source.relative),
        );
        return;
    };
    let name = ident.to_string();
    let qn = format!("{module}::{name}");
    let id = add_node(
        context,
        "function",
        &qn,
        &name,
        source,
        item.span(),
        "macro",
    );
    context.macros.insert(qn, id.clone());
    crate::adapters::rust::relationships::define_and_contain(
        context,
        owner,
        &id,
        item.span(),
        &source.relative,
    );
}
