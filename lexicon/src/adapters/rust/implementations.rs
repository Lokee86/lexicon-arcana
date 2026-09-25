use crate::adapters::rust::declarations::add_node;
use crate::adapters::rust::function_index::{self, Registration};
use crate::adapters::rust::model::{Context, FunctionBody, MethodInfo, PendingImpl, SourceFile};
use crate::adapters::rust::paths::span_value;
use crate::adapters::rust::resolve;
use crate::adapters::rust::syntax::{normalized_tokens, self_type_name};
use syn::spanned::Spanned;

pub(crate) fn process(
    context: &mut Context,
    item: &syn::ItemImpl,
    owner: &str,
    module: &str,
    crate_qn: &str,
    source: &SourceFile,
) {
    let self_type = crate::adapters::rust::syntax::type_tokens(&item.self_ty);
    let type_name = self_type_name(&item.self_ty);
    let trait_path = item
        .trait_
        .as_ref()
        .map(|(_, path, _)| normalized_tokens(path));
    if let Some(trait_path) = &trait_path {
        context.pending_impls.push(PendingImpl {
            owner_id: owner.into(),
            self_type: self_type.clone(),
            trait_path: trait_path.clone(),
            module_qn: module.into(),
            crate_qn: crate_qn.into(),
            expression: format!("impl {trait_path} for {self_type}"),
            span: span_value(item.span(), &source.relative),
        });
    }
    let suffix = trait_path
        .as_deref()
        .map(|value| format!("::{value}"))
        .unwrap_or_default();
    for impl_item in &item.items {
        if let syn::ImplItem::Fn(method) = impl_item {
            let name = method.sig.ident.to_string();
            let qn = format!("{module}::{type_name}{suffix}::{name}");
            let id = add_node(
                context,
                "method",
                &qn,
                &name,
                source,
                method.span(),
                "impl-method",
            );
            context.symbols.insert(qn.clone(), id.clone());
            context.function_qn_by_id.insert(id.clone(), qn.clone());
            crate::adapters::rust::relationships::define_and_contain(
                context,
                owner,
                &id,
                method.span(),
                &source.relative,
            );
            context.methods.push(MethodInfo {
                id: id.clone(),
                self_type: self_type.clone(),
                trait_path: trait_path.clone(),
                name,
                module_qn: module.into(),
                crate_qn: crate_qn.into(),
            });
            function_index::register(
                context,
                Registration {
                    id,
                    qn,
                    module_qn: module,
                    crate_qn,
                    source_path: &source.relative,
                    signature: &method.sig,
                    body: FunctionBody::Block(method.block.clone()),
                    self_type: Some(self_type.clone()),
                    trait_path: trait_path.clone(),
                },
            );
        }
    }
}

pub(crate) fn finalize(context: &mut Context) {
    let methods = context.methods.clone();
    for method in methods {
        let Some(function) = context.functions.get(&method.id).cloned().or_else(|| {
            Some(crate::adapters::rust::model::FunctionInfo {
                id: method.id.clone(),
                qn: context
                    .function_qn_by_id
                    .get(&method.id)
                    .cloned()
                    .unwrap_or_default(),
                module_qn: method.module_qn.clone(),
                crate_qn: method.crate_qn.clone(),
                source_path: String::new(),
                body: FunctionBody::Expr(syn::parse_quote!(())),
                parameters: Vec::new(),
                return_type: None,
                self_type: Some(method.self_type.clone()),
                trait_path: method.trait_path.clone(),
                generic_bounds: Default::default(),
            })
        }) else {
            continue;
        };
        let type_ids = resolve::resolve_type_ids(context, &method.self_type, &function);
        let trait_ids = method
            .trait_path
            .as_deref()
            .map(|path| resolve::resolve_trait_ids(context, path, &function))
            .unwrap_or_default();
        for type_id in &type_ids {
            context.facts.add_edge(type_id, &method.id, "defines", None);
            context
                .method_index
                .entry((type_id.clone(), method.name.clone()))
                .or_default()
                .push(method.id.clone());
            for trait_id in &trait_ids {
                context
                    .trait_method_index
                    .entry((trait_id.clone(), method.name.clone()))
                    .or_default()
                    .push(method.id.clone());
                let contract_ids = context
                    .trait_method_index
                    .get(&(trait_id.clone(), method.name.clone()))
                    .cloned()
                    .unwrap_or_default();
                for contract_id in contract_ids {
                    if context.trait_method_ids.contains(&contract_id) && contract_id != method.id {
                        context
                            .facts
                            .add_edge(&method.id, &contract_id, "overrides", None);
                    }
                }
            }
        }
        if method.self_type == "Self" {
            for trait_id in trait_ids {
                context
                    .trait_method_index
                    .entry((trait_id, method.name.clone()))
                    .or_default()
                    .push(method.id.clone());
            }
        }
    }
    for values in context
        .method_index
        .values_mut()
        .chain(context.trait_method_index.values_mut())
    {
        values.sort();
        values.dedup();
    }
    let implementations = context.pending_impls.clone();
    for item in implementations {
        let function = crate::adapters::rust::model::FunctionInfo {
            id: String::new(),
            qn: String::new(),
            module_qn: item.module_qn.clone(),
            crate_qn: item.crate_qn.clone(),
            source_path: String::new(),
            body: FunctionBody::Expr(syn::parse_quote!(())),
            parameters: Vec::new(),
            return_type: None,
            self_type: Some(item.self_type.clone()),
            trait_path: Some(item.trait_path.clone()),
            generic_bounds: Default::default(),
        };
        let types = resolve::resolve_type_ids(context, &item.self_type, &function);
        let traits = resolve::resolve_trait_ids(context, &item.trait_path, &function);
        if types.is_empty() || traits.is_empty() {
            context.facts.add_unresolved(
                &item.owner_id,
                "implements",
                &item.expression,
                if resolve::is_external_path(context, &item.trait_path, &function)
                    || !item
                        .trait_path
                        .split('<')
                        .next()
                        .unwrap_or_default()
                        .contains("::")
                {
                    "external-target"
                } else {
                    "missing-target"
                },
                item.span.clone(),
            );
        } else {
            for type_id in &types {
                for trait_id in &traits {
                    context
                        .facts
                        .add_edge(type_id, trait_id, "implements", item.span.clone());
                    context
                        .type_traits
                        .entry(type_id.clone())
                        .or_default()
                        .insert(trait_id.clone());
                }
            }
        }
    }
}
