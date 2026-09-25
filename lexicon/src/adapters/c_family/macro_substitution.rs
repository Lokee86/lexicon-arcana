use super::{
    macro_syntax::{identifier_part, identifier_start, matching_delimiter, skip_quoted},
    model::{Declaration, MacroCallExpression},
    syntax::normalize_qualified,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub fn invocation_bindings(
    macro_declaration: &Declaration,
    arguments: &[String],
) -> Option<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    if !macro_declaration.macro_function {
        return Some(result);
    }
    if macro_declaration.macro_parameters.len() != arguments.len() {
        return None;
    }
    for (parameter, argument) in macro_declaration.macro_parameters.iter().zip(arguments) {
        if parameter == "__VA_ARGS__" {
            return None;
        }
        result.insert(parameter.clone(), argument.trim().to_owned());
    }
    Some(result)
}

pub fn substitute_call(
    call: &MacroCallExpression,
    bindings: &BTreeMap<String, String>,
) -> MacroCallExpression {
    let mut value = call.clone();
    value.callee = substitute_callee(&call.callee, bindings);
    value.arguments = call
        .arguments
        .iter()
        .map(|argument| substitute_tokens(argument, bindings))
        .collect();
    value
}

pub fn argument_candidates(arguments: &[String]) -> Vec<String> {
    arguments
        .iter()
        .map(|argument| simple_identifier(argument).unwrap_or_default())
        .collect()
}

pub fn unsupported_substitution(macro_declaration: &Declaration) -> bool {
    let replacement = macro_declaration
        .attributes
        .get("replacement")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let bytes = replacement.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if matches!(bytes[index], b'"' | b'\'') {
            index = skip_quoted(replacement, index);
            continue;
        }
        if bytes[index] == b'#' {
            return true;
        }
        if identifier_start(bytes[index]) {
            let start = index;
            index += 1;
            while index < bytes.len() && identifier_part(bytes[index]) {
                index += 1;
            }
            if matches!(&replacement[start..index], "__VA_ARGS__" | "__VA_OPT__") {
                return true;
            }
            continue;
        }
        index += 1;
    }
    false
}

fn substitute_callee(callee: &str, bindings: &BTreeMap<String, String>) -> String {
    if let Some(replacement) = bindings.get(callee) {
        return normalize_qualified(strip_wrapping_parentheses(replacement));
    }
    normalize_qualified(&substitute_tokens(callee, bindings))
}

fn substitute_tokens(expression: &str, bindings: &BTreeMap<String, String>) -> String {
    if bindings.is_empty() || expression.is_empty() {
        return expression.to_owned();
    }
    let bytes = expression.as_bytes();
    let mut output = String::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if matches!(bytes[index], b'"' | b'\'') {
            let end = skip_quoted(expression, index).min(bytes.len());
            output.push_str(&expression[index..end]);
            index = end;
            continue;
        }
        if !identifier_start(bytes[index]) {
            output.push(bytes[index] as char);
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len() && identifier_part(bytes[index]) {
            index += 1;
        }
        let identifier = &expression[start..index];
        if let Some(replacement) = bindings.get(identifier) {
            output.push('(');
            output.push_str(replacement.trim());
            output.push(')');
        } else {
            output.push_str(identifier);
        }
    }
    output
}

fn strip_wrapping_parentheses(mut expression: &str) -> &str {
    expression = expression.trim();
    while expression.len() >= 2 && expression.starts_with('(') {
        let Some(close) = matching_delimiter(expression, 0, b'(', b')') else {
            break;
        };
        if close != expression.len() - 1 {
            break;
        }
        expression = expression[1..close].trim();
    }
    expression
}

fn simple_identifier(expression: &str) -> Option<String> {
    let expression = strip_wrapping_parentheses(expression);
    let bytes = expression.as_bytes();
    if bytes.is_empty() || !identifier_start(bytes[0]) {
        return None;
    }
    if bytes.iter().skip(1).all(|value| identifier_part(*value)) {
        Some(expression.to_owned())
    } else {
        None
    }
}
