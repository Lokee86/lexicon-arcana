use serde_json::json;

use super::dependencies::dependency_attributes;
use super::facts::Facts;
use super::model::ParsedFile;
use super::parser::{declaration_for_statement, find_imports};
use super::semantic::{SemanticModel, context_for_statement};

pub fn process_imports(
    facts: &mut Facts,
    files: &[ParsedFile],
    file_index: usize,
    _model: &SemanticModel,
) {
    let file = &files[file_index];
    let mut ordinal = 0_usize;
    for statement in &file.statements {
        let owner = owner_for_statement(files, file_index, statement);
        for reference in find_imports(statement, &file.path, &file.project_root) {
            ordinal += 1;
            let identity = format!("{}::import::{ordinal}::{}", file.path, reference.expression);
            let import_id = facts.add_node(
                "import",
                &reference.expression,
                &file.path,
                &identity,
                &identity,
                Some(reference.span.clone()),
                None,
                Some(json!({
                    "expression": reference.expression,
                    "loader": reference.loader,
                    "static": reference.is_static,
                    "resolved_path": (!reference.path.is_empty()).then_some(reference.path.clone()),
                })),
            );
            facts.add_edge(
                &owner,
                &import_id,
                "imports",
                Some(reference.span.clone()),
                None,
            );

            if reference.is_static && !reference.path.is_empty() {
                if let Some(target) = facts.module_by_path.get(&reference.path).cloned() {
                    facts.add_edge(
                        &import_id,
                        &target,
                        "references",
                        Some(reference.span.clone()),
                        None,
                    );
                    facts.add_edge(
                        &file.module_id,
                        &target,
                        "depends-on",
                        Some(reference.span.clone()),
                        Some(dependency_attributes("local", &reference.expression, true)),
                    );
                } else {
                    let reason = if std::path::Path::new(&reference.path)
                        .extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|value| value.eq_ignore_ascii_case("gd"))
                    {
                        "missing-target"
                    } else {
                        "external-target"
                    };
                    facts.add_unresolved(
                        &owner,
                        "imports",
                        &reference.expression,
                        reason,
                        Some(reference.span),
                        None,
                    );
                }
            } else if reference.is_static {
                facts.add_unresolved(
                    &owner,
                    "imports",
                    &reference.expression,
                    "external-target",
                    Some(reference.span),
                    None,
                );
            } else {
                facts.add_unresolved(
                    &owner,
                    "imports",
                    &reference.expression,
                    "dynamic-target",
                    Some(reference.span),
                    None,
                );
            }
        }
    }
}

pub fn owner_for_statement(
    files: &[ParsedFile],
    file_index: usize,
    statement: &super::model::Statement,
) -> String {
    let file = &files[file_index];
    if let Some(declaration) = declaration_for_statement(file, statement)
        && !declaration.node_id.is_empty()
    {
        return declaration.node_id.clone();
    }
    let context = context_for_statement(files, file_index, statement);
    if context.function_id.is_empty() {
        context.owner_id
    } else {
        context.function_id
    }
}
