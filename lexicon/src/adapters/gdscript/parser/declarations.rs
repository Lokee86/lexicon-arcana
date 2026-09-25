use std::collections::BTreeMap;

use crate::AdapterError;

use super::super::lexer::{lex, make_statements, span};
use super::super::model::{Declaration, ParsedFile, Statement, Token, TokenKind};
use super::super::syntax::{
    join_tokens, matching_paren, split_arguments, top_level_assignment, top_level_token,
};
use super::is_declaration_keyword;

pub fn parse_file(path: &str, content: &[u8]) -> Result<ParsedFile, AdapterError> {
    let source = std::str::from_utf8(content)
        .map_err(|error| AdapterError::new(format!("non-UTF-8 GDScript source: {error}")))?;
    let tokens = lex(source)?;
    let statements = make_statements(&tokens);
    let mut declarations = statements
        .iter()
        .filter_map(|statement| parse_declaration(path, statement))
        .collect::<Vec<_>>();
    declarations.extend(parse_anonymous_functions(path, &tokens));
    declarations
        .sort_by_key(|declaration| (declaration.span.start_line, declaration.span.start_column));
    Ok(ParsedFile {
        path: path.into(),
        project_root: ".".into(),
        content: content.to_vec(),
        statements,
        declarations,
        module_id: String::new(),
        class_id: String::new(),
        script_owner_id: String::new(),
    })
}

fn parse_anonymous_functions(path: &str, tokens: &[Token]) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.text != "func"
            || tokens.get(index + 1).map(|value| value.text.as_str()) != Some("(")
        {
            continue;
        }
        let Some(close) = matching_paren(tokens, index + 1) else {
            continue;
        };
        let mut end = close;
        if let Some(arrow) = index_of_after(tokens, "->", close)
            && let Some(colon) = index_of_after(tokens, ":", arrow)
        {
            end = colon;
        } else if let Some(colon) = index_of_after(tokens, ":", close) {
            end = colon;
        }
        let mut declaration = Declaration::new(
            "lambda".into(),
            "function".into(),
            token.column.saturating_sub(1) as usize,
            span(path, token, &tokens[end]),
        );
        declaration.name = format!("<lambda@{}:{}>", token.line, token.column);
        declaration.name_index = index;
        parse_function_signature(&mut declaration, &tokens[index..=end], 0);
        declarations.push(declaration);
    }
    declarations
}

fn parse_declaration(path: &str, statement: &Statement) -> Option<Declaration> {
    let keyword_index = statement.tokens.iter().position(|token| {
        token.kind == TokenKind::Identifier && is_declaration_keyword(&token.text)
    })?;
    let keyword = statement.tokens[keyword_index].text.clone();
    let kind = match keyword.as_str() {
        "class_name" | "class" => "type",
        "func" => "function",
        "signal" => "signal",
        "const" => "constant",
        "var" => "variable",
        "extends" => "extends",
        _ => return None,
    };
    let mut declaration = Declaration::new(
        keyword.clone(),
        kind.into(),
        statement.indent,
        span(path, &statement.start, &statement.end),
    );
    declaration.is_static = statement.tokens[..keyword_index]
        .iter()
        .any(|token| token.text == "static");
    declaration.is_async = statement.tokens[..keyword_index]
        .iter()
        .any(|token| token.text == "async");

    let name_index = keyword_index + 1;
    if let Some(token) = statement.tokens.get(name_index)
        && token.kind == TokenKind::Identifier
    {
        declaration.name = token.text.clone();
        declaration.name_index = name_index;
    }

    if keyword == "extends" {
        declaration.name = "extends".into();
        declaration.extends = join_until(&statement.tokens[keyword_index + 1..], ":");
        return Some(declaration);
    }
    if declaration.name.is_empty() {
        return None;
    }

    if keyword == "func" {
        parse_function_signature(&mut declaration, &statement.tokens, name_index);
    }
    if matches!(keyword.as_str(), "class_name" | "class")
        && let Some(index) = statement
            .tokens
            .iter()
            .position(|token| token.text == "extends")
    {
        declaration.extends = join_until(&statement.tokens[index + 1..], ":");
    }
    if matches!(keyword.as_str(), "var" | "const") {
        parse_value_declaration(&mut declaration, &statement.tokens, name_index);
    }
    Some(declaration)
}

fn parse_function_signature(declaration: &mut Declaration, tokens: &[Token], name_index: usize) {
    let Some(open) = tokens
        .iter()
        .enumerate()
        .skip(name_index + 1)
        .find_map(|(index, token)| (token.text == "(").then_some(index))
    else {
        return;
    };
    let Some(close) = matching_paren(tokens, open) else {
        return;
    };
    let parameters = split_arguments(&tokens[open + 1..close]);
    declaration.parameters = parameters.iter().map(|value| join_tokens(value)).collect();
    let (names, types, defaults) = parameter_details(&parameters);
    declaration.parameter_names = names;
    declaration.parameter_types = types;
    declaration.parameter_defaults = defaults;

    if let Some(arrow) = index_of_after(tokens, "->", close) {
        declaration.return_type = join_until(&tokens[arrow + 1..], ":").trim().into();
    }
}

fn parse_value_declaration(declaration: &mut Declaration, tokens: &[Token], name_index: usize) {
    let equals = tokens
        .iter()
        .enumerate()
        .skip(name_index + 1)
        .find_map(|(index, token)| matches!(token.text.as_str(), "=" | ":=").then_some(index));
    if let Some(colon) = index_of_after(tokens, ":", name_index)
        && equals.is_none_or(|equals| colon < equals)
    {
        let end = equals.unwrap_or(tokens.len());
        declaration.type_name = join_tokens(&tokens[colon + 1..end]).trim().into();
        if !declaration.type_name.is_empty() {
            declaration.attributes.insert(
                "type".into(),
                serde_json::Value::String(declaration.type_name.clone()),
            );
        }
    }
    if let Some(equals) = equals
        && equals + 1 < tokens.len()
    {
        declaration.initializer = tokens[equals + 1..].to_vec();
    }
    declaration.preload_path = parse_static_load_path(&tokens[name_index + 1..]);
}

#[allow(clippy::type_complexity)]
fn parameter_details(
    parameters: &[Vec<Token>],
) -> (
    Vec<String>,
    BTreeMap<String, String>,
    BTreeMap<String, Vec<Token>>,
) {
    let mut names = Vec::new();
    let mut types = BTreeMap::new();
    let mut defaults = BTreeMap::new();

    for parameter in parameters {
        let equals = top_level_assignment(parameter);
        let left = equals.map_or(parameter.as_slice(), |index| &parameter[..index]);
        let colon = top_level_token(left, ":");
        let name_tokens = colon.map_or(left, |index| &left[..index]);
        let name = join_tokens(name_tokens).trim().to_owned();
        if name.is_empty() {
            continue;
        }
        names.push(name.clone());
        if let Some(colon) = colon
            && colon + 1 < left.len()
        {
            types.insert(name.clone(), join_tokens(&left[colon + 1..]).trim().into());
        }
        if let Some(equals) = equals
            && equals + 1 < parameter.len()
        {
            defaults.insert(name, parameter[equals + 1..].to_vec());
        }
    }
    (names, types, defaults)
}

fn parse_static_load_path(tokens: &[Token]) -> String {
    let Some(equals) = top_level_assignment(tokens) else {
        return String::new();
    };
    if equals + 4 >= tokens.len()
        || !matches!(tokens[equals + 1].text.as_str(), "preload" | "load")
        || tokens[equals + 2].text != "("
    {
        return String::new();
    }
    let Some(close) = matching_paren(tokens, equals + 2) else {
        return String::new();
    };
    if close != equals + 4
        || close != tokens.len() - 1
        || tokens[equals + 3].kind != TokenKind::String
    {
        return String::new();
    }
    super::normalize_import_path(&tokens[equals + 3].text).unwrap_or_default()
}

fn index_of_after(tokens: &[Token], text: &str, start: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, token)| (token.text == text).then_some(index))
}

fn join_until(tokens: &[Token], stop: &str) -> String {
    let end = tokens
        .iter()
        .position(|token| token.text == stop)
        .unwrap_or(tokens.len());
    join_tokens(&tokens[..end])
}
