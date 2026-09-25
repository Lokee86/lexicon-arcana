use super::{
    declaration_helpers::{
        add_declaration, declaration_type, declarator_name, internal_linkage,
        is_function_pointer_declarator,
    },
    model::{CallableShape, ExtractionContext, SourceFile},
    syntax::{
        first_descendant, last_qualified_part, named_children, node_text, normalize_space, qualify,
    },
};
use serde_json::{Map, json};
use tree_sitter::Node;

pub fn handle_function<'tree>(
    file: &mut SourceFile,
    node: Node<'tree>,
    context: &ExtractionContext,
    source: &[u8],
    definition: bool,
) -> Option<(usize, Node<'tree>)> {
    let declarator = node
        .child_by_field_name("declarator")
        .and_then(|value| first_descendant(value, &["function_declarator"]))
        .or_else(|| first_descendant(node, &["function_declarator"]))?;
    let index = handle_function_declarator(file, node, declarator, context, source, definition)?;
    Some((index, declarator))
}

pub fn handle_function_declarator(
    file: &mut SourceFile,
    node: Node<'_>,
    declarator: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
    definition: bool,
) -> Option<usize> {
    let name_text = declarator_name(declarator, source);
    if name_text.is_empty() {
        return None;
    }
    let name = last_qualified_part(&name_text);
    let kind = callable_kind(&name, context);
    let qualified = qualify(&context.container_qualified, &name_text);
    let signature = normalize_space(node_text(declarator, source));
    let mut attributes = Map::new();
    attributes.insert("definition".into(), json!(definition));
    if internal_linkage(file, node, context, source) {
        attributes.insert("linkage".into(), json!("internal"));
    }
    if context.template {
        attributes.insert("template".into(), json!(true));
    }
    if !definition && node_text(node, source).contains("virtual") {
        attributes.insert("virtual".into(), json!(true));
    }

    let index = add_declaration(
        file, node, context, kind, name, qualified, signature, true, definition, attributes,
    );
    add_parameters(file, declarator, index, source);
    file.declarations[index].callable_shape = callable_parameter_shape(file, declarator, source);
    Some(index)
}

pub fn function_declarator_is_pointer(function: Node<'_>) -> bool {
    is_function_pointer_declarator(function)
}

fn add_parameters(
    file: &mut SourceFile,
    declarator: Node<'_>,
    callable_index: usize,
    source: &[u8],
) {
    let Some(list) = first_descendant(declarator, &["parameter_list"]) else {
        return;
    };
    let callable = file.declarations[callable_index].clone();
    let mut parameter_count = 0usize;
    for child in named_children(list) {
        if !matches!(
            child.kind(),
            "parameter_declaration" | "optional_parameter_declaration"
        ) {
            continue;
        }
        let Some(declarator) = child.child_by_field_name("declarator") else {
            parameter_count += 1;
            continue;
        };
        let name = last_qualified_part(&declarator_name(declarator, source));
        if name.is_empty() {
            parameter_count += 1;
            continue;
        }
        let context = ExtractionContext {
            container_id: callable.id.clone(),
            container_qualified: callable.qualified_name.clone(),
            type_id: callable.parent_type_id.clone(),
            callable_id: callable.id.clone(),
            callable_scope: callable.qualified_name.clone(),
            ..Default::default()
        };
        let mut attributes = Map::new();
        attributes.insert("index".into(), json!(parameter_count));
        attributes.insert("type".into(), json!(declaration_type(child, source)));
        if first_descendant(declarator, &["function_declarator"]).is_some() {
            attributes.insert("function_pointer".into(), json!(true));
        }
        add_declaration(
            file,
            child,
            &context,
            "parameter",
            name.clone(),
            format!("{}::{name}", callable.qualified_name),
            format!("parameter#{parameter_count}"),
            false,
            true,
            attributes,
        );
        parameter_count += 1;
    }
    file.declarations[callable_index]
        .attributes
        .insert("parameter_count".into(), json!(parameter_count));
}

fn callable_kind(name: &str, context: &ExtractionContext) -> &'static str {
    if context.type_id.is_empty() {
        "function"
    } else if name == context.type_name {
        "constructor"
    } else {
        "method"
    }
}

fn callable_parameter_shape(
    file: &SourceFile,
    declarator: Node<'_>,
    source: &[u8],
) -> Option<CallableShape> {
    let list = first_descendant(declarator, &["parameter_list"])?;
    if parameter_list_is_void(list, source) {
        return Some(CallableShape {
            minimum: 0,
            maximum: Some(0),
            variadic: false,
        });
    }
    let children = named_children(list);
    if file.language == "c" && children.iter().all(|child| child.kind() == "comment") {
        return None;
    }

    let text = normalize_space(node_text(list, source));
    let trailing_variadic = parameter_text_has_trailing_variadic(&text);
    if text.contains("...") && !trailing_variadic {
        return None;
    }

    let mut minimum = 0usize;
    let mut maximum = 0usize;
    let mut optional = false;
    let mut variadic = false;
    for child in children {
        match child.kind() {
            "comment" => {}
            "variadic_parameter" => {
                if variadic || optional {
                    return None;
                }
                variadic = true;
            }
            "parameter_declaration" => {
                if variadic || optional || child.child_by_field_name("type").is_none() {
                    return None;
                }
                minimum += 1;
                maximum += 1;
            }
            "optional_parameter_declaration" => {
                if variadic
                    || child.child_by_field_name("type").is_none()
                    || child.child_by_field_name("default_value").is_none()
                {
                    return None;
                }
                optional = true;
                maximum += 1;
            }
            _ => return None,
        }
    }
    if trailing_variadic {
        if optional {
            return None;
        }
        variadic = true;
    }
    Some(CallableShape {
        minimum,
        maximum: (!variadic).then_some(maximum),
        variadic,
    })
}

fn parameter_list_is_void(list: Node<'_>, source: &[u8]) -> bool {
    let values = named_children(list)
        .into_iter()
        .filter(|child| child.kind() != "comment")
        .collect::<Vec<_>>();
    values.len() == 1
        && values[0].kind() == "parameter_declaration"
        && values[0].child_by_field_name("declarator").is_none()
        && values[0]
            .child_by_field_name("type")
            .is_some_and(|value| normalize_space(node_text(value, source)) == "void")
}

fn parameter_text_has_trailing_variadic(text: &str) -> bool {
    let inner = text
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or("")
        .trim();
    inner == "..." || inner.ends_with(",...") || inner.ends_with(", ...")
}
