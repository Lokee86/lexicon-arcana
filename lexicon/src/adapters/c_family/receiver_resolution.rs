use super::{
    model::{CallObservation, Declaration},
    resolution::DeclarationIndex,
    syntax::normalize_space,
};

pub fn direct_receiver_type_id(
    index: &DeclarationIndex<'_>,
    observation: &CallObservation,
) -> String {
    if !observation.member
        || observation.receiver.is_empty()
        || matches!(observation.receiver.as_str(), "this" | "self")
    {
        return String::new();
    }
    let Some(source) = index.by_id.get(&observation.source_id).copied() else {
        return String::new();
    };
    if source.file_language != "cpp" {
        return String::new();
    }

    let accept =
        |value: &Declaration| matches!(value.kind.as_str(), "parameter" | "variable" | "field");
    let key = container_key(&observation.source_id, &observation.receiver);
    let mut candidates = index
        .by_container_name
        .get(&key)
        .into_iter()
        .flatten()
        .copied()
        .filter(|value| accept(value))
        .collect::<Vec<_>>();

    if !source.parent_type_id.is_empty() {
        let key = container_key(&source.parent_type_id, &observation.receiver);
        candidates.extend(
            index
                .by_container_name
                .get(&key)
                .into_iter()
                .flatten()
                .copied()
                .filter(|value| {
                    value.kind == "field"
                        && index
                            .visibility
                            .declaration_visible(&observation.path, value)
                }),
        );
    }
    candidates.sort_by(|left, right| left.id.cmp(&right.id));
    candidates.dedup_by(|left, right| left.id == right.id);
    if candidates.len() != 1 {
        return String::new();
    }

    let type_text = candidates[0]
        .attributes
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let type_name = direct_receiver_type_name(type_text);
    if type_name.is_empty() {
        return String::new();
    }
    let types = index.resolve(
        &type_name,
        &source.qualified_name,
        &observation.path,
        |value| {
            value.kind == "type"
                && !value
                    .attributes
                    .get("alias")
                    .and_then(|attribute| attribute.as_bool())
                    .unwrap_or(false)
        },
    );
    if types.len() == 1 {
        types[0].id.clone()
    } else {
        String::new()
    }
}

fn direct_receiver_type_name(value: &str) -> String {
    let value = normalize_space(value);
    if value.is_empty() || value.contains(['<', '>', '(', ')', '[', ']', ',']) {
        return String::new();
    }
    let value = value.replace(['*', '&'], "");
    let parts = value
        .split_whitespace()
        .filter(|part| !matches!(*part, "const" | "volatile" | "struct" | "class" | "enum"))
        .collect::<Vec<_>>();
    if parts.len() == 1 {
        super::syntax::normalize_qualified(parts[0])
    } else {
        String::new()
    }
}

pub fn container_key(container: &str, name: &str) -> String {
    format!("{container}\0{name}")
}

#[cfg(test)]
mod tests {
    use super::direct_receiver_type_name;

    #[test]
    fn receiver_type_normalization_is_conservative() {
        assert_eq!(direct_receiver_type_name("const Widget *"), "Widget");
        assert_eq!(direct_receiver_type_name("struct Widget &"), "Widget");
        assert_eq!(direct_receiver_type_name("Widget<int> *"), "");
        assert_eq!(direct_receiver_type_name("unsigned long"), "");
    }
}
