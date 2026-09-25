use super::model::{Token, TokenKind};

pub fn matching_paren(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0_i32;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.text.as_str() {
            "(" => depth += 1,
            ")" => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn split_arguments(tokens: &[Token]) -> Vec<Vec<Token>> {
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let (mut start, mut depth) = (0_usize, 0_i32);
    for index in 0..=tokens.len() {
        if index == tokens.len() || (tokens[index].text == "," && depth == 0) {
            let value = trim_expression(&tokens[start..index]);
            if !value.is_empty() {
                result.push(value.to_vec());
            }
            start = index + 1;
            continue;
        }
        match tokens[index].text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth > 0 => depth -= 1,
            _ => {}
        }
    }
    result
}

pub fn trim_expression(mut tokens: &[Token]) -> &[Token] {
    while matches!(
        tokens.first().map(|token| token.text.as_str()),
        Some("await" | "return")
    ) {
        tokens = &tokens[1..];
    }
    while tokens.len() > 1
        && tokens[0].text == "("
        && tokens[tokens.len() - 1].text == ")"
        && matching_paren(tokens, 0) == Some(tokens.len() - 1)
    {
        tokens = &tokens[1..tokens.len() - 1];
    }
    tokens
}

pub fn simple_identifier(tokens: &[Token]) -> Option<&str> {
    let tokens = trim_expression(tokens);
    (tokens.len() == 1 && tokens[0].kind == TokenKind::Identifier)
        .then_some(tokens[0].text.as_str())
}

pub fn string_literal(tokens: &[Token]) -> Option<String> {
    let mut tokens = trim_expression(tokens);
    if tokens.len() == 2 && tokens[0].text == "&" {
        tokens = &tokens[1..];
    }
    if tokens.len() != 1 || tokens[0].kind != TokenKind::String || tokens[0].text.len() < 2 {
        return None;
    }
    let value = &tokens[0].text;
    if (value.starts_with("\"\"\"") || value.starts_with("'''")) && value.len() >= 6 {
        return Some(value[3..value.len() - 3].to_owned());
    }
    Some(value[1..value.len() - 1].to_owned())
}

pub fn join_tokens(tokens: &[Token]) -> String {
    tokens
        .iter()
        .filter(|token| token.text != "\n")
        .map(|token| token.text.as_str())
        .collect()
}

pub fn top_level_token(tokens: &[Token], text: &str) -> Option<usize> {
    top_level_token_after(tokens, text, None)
}

pub fn top_level_token_after(tokens: &[Token], text: &str, start: Option<usize>) -> Option<usize> {
    let mut depth = 0_i32;
    let first = start.map_or(0, |value| value + 1);
    for (index, token) in tokens.iter().enumerate().skip(first) {
        match token.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth > 0 => depth -= 1,
            _ if depth == 0 && token.text == text => return Some(index),
            _ => {}
        }
    }
    None
}

pub fn top_level_assignment(tokens: &[Token]) -> Option<usize> {
    let mut depth = 0_i32;
    for (index, token) in tokens.iter().enumerate() {
        match token.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth > 0 => depth -= 1,
            "=" | ":=" if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

pub fn property_chain(tokens: &[Token]) -> Option<Vec<String>> {
    let tokens = trim_expression(tokens);
    if tokens.len() < 3 || tokens.len().is_multiple_of(2) {
        return None;
    }
    let mut parts = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if index.is_multiple_of(2) {
            if token.kind != TokenKind::Identifier {
                return None;
            }
            parts.push(token.text.clone());
        } else if token.text != "." {
            return None;
        }
    }
    Some(parts)
}
