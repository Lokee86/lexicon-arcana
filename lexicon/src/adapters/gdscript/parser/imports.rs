use super::super::lexer::span;
use super::super::model::{ImportReference, Statement, TokenKind};
use super::super::syntax::{join_tokens, matching_paren};

pub fn find_imports(
    statement: &Statement,
    source_path: &str,
    project_root: &str,
) -> Vec<ImportReference> {
    let mut references = Vec::new();
    for index in 0..statement.tokens.len().saturating_sub(2) {
        let current = &statement.tokens[index];
        if current.kind != TokenKind::Identifier
            || !matches!(current.text.as_str(), "preload" | "load")
            || statement.tokens[index + 1].text != "("
        {
            continue;
        }
        let Some(close) = matching_paren(&statement.tokens, index + 1) else {
            continue;
        };
        let args = &statement.tokens[index + 2..close];
        let expression = join_tokens(args);
        let mut reference = ImportReference {
            loader: current.text.clone(),
            expression,
            path: String::new(),
            is_static: false,
            span: span(source_path, current, &statement.tokens[close]),
        };
        if args.len() == 1 && args[0].kind == TokenKind::String {
            reference.is_static = true;
            if let Some(path) = normalize_import_path(&args[0].text) {
                reference.path = project_resource_path(project_root, &path);
            }
        }
        references.push(reference);
    }
    references
}

pub fn normalize_import_path(expression: &str) -> Option<String> {
    let mut value = expression.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value = &value[1..value.len() - 1];
    }
    let value = value.strip_prefix("res://")?;
    let path = super::super::facts::normalize_path(value);
    (path != "." && !path.starts_with("../")).then_some(path)
}

pub fn project_resource_path(project_root: &str, resource_path: &str) -> String {
    let resource = super::super::facts::normalize_path(resource_path);
    let root = super::super::facts::normalize_path(project_root);
    if matches!(root.as_str(), "." | "") {
        resource
    } else {
        super::super::facts::normalize_path(&format!("{root}/{resource}"))
    }
}
