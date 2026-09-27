use rustpython_parser::ast;

use super::super::super::model::FunctionInfo;

pub(super) fn function_returns_parameter(info: &FunctionInfo, parameter: &str) -> bool {
    !info.return_expressions.is_empty()
        && info.return_expressions.iter().all(|expression| {
            matches!(
                expression,
                ast::Expr::Name(value) if value.id.as_str() == parameter
            )
        })
}

pub(super) fn parameter_names(expression: &ast::Expr) -> Vec<String> {
    match expression {
        ast::Expr::Constant(value) => match &value.value {
            ast::Constant::Str(value) => value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        },
        ast::Expr::List(value) => constant_strings(&value.elts),
        ast::Expr::Tuple(value) => constant_strings(&value.elts),
        _ => Vec::new(),
    }
}

fn constant_strings(values: &[ast::Expr]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| match value {
            ast::Expr::Constant(value) => match &value.value {
                ast::Constant::Str(value) => Some(value.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

pub(super) fn parameter_values(
    expression: &ast::Expr,
    width: usize,
    index: usize,
) -> Vec<ast::Expr> {
    let rows = match expression {
        ast::Expr::List(value) => &value.elts,
        ast::Expr::Tuple(value) => &value.elts,
        ast::Expr::Set(value) => &value.elts,
        _ => return Vec::new(),
    };
    rows.iter()
        .filter_map(|row| {
            if width <= 1 {
                return Some(row.clone());
            }
            match row {
                ast::Expr::List(value) => value.elts.get(index).cloned(),
                ast::Expr::Tuple(value) => value.elts.get(index).cloned(),
                _ => None,
            }
        })
        .collect()
}
