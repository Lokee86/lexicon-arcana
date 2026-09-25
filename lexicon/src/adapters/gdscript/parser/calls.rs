use super::super::lexer::span;
use super::super::model::{CallReference, Statement, Token, TokenKind};
use super::super::syntax::{join_tokens, matching_paren, split_arguments, trim_expression};
use super::is_call_keyword;

pub fn find_calls(statement: &Statement, path: &str) -> Vec<CallReference> {
    find_calls_in_tokens(&statement.tokens, path)
}

pub fn find_calls_in_tokens(tokens: &[Token], path: &str) -> Vec<CallReference> {
    let mut calls = Vec::new();
    for index in 0..tokens.len().saturating_sub(1) {
        let current = &tokens[index];
        if current.kind != TokenKind::Identifier
            || tokens[index + 1].text != "("
            || is_call_keyword(&current.text)
            || matches!(current.text.as_str(), "preload" | "load")
            || is_call_declaration_name(tokens, index)
        {
            continue;
        }
        let Some(close) = matching_paren(tokens, index + 1) else {
            continue;
        };
        if close <= index + 1 {
            continue;
        }
        let mut start = index;
        let mut receiver = Vec::new();
        if index > 0 && tokens[index - 1].text == "." {
            start = receiver_start(tokens, index - 1);
            receiver = tokens[start..index - 1].to_vec();
        }
        calls.push(CallReference {
            callee: join_tokens(&tokens[start..=index]),
            name: current.text.clone(),
            receiver,
            args: split_arguments(&tokens[index + 2..close]),
            expression: join_tokens(&tokens[start..=close]),
            span: span(path, &tokens[start], &tokens[close]),
        });
    }
    calls
}

pub fn terminal_call(tokens: &[Token]) -> Option<CallReference> {
    let tokens = trim_expression(tokens);
    if tokens.len() < 3 || tokens.last()?.text != ")" {
        return None;
    }
    find_calls_in_tokens(tokens, "")
        .into_iter()
        .rev()
        .find(|call| {
            call.span.end_line == tokens.last().unwrap().end_line
                && call.span.end_column == tokens.last().unwrap().end_column
        })
}

fn receiver_start(tokens: &[Token], dot: usize) -> usize {
    let mut depth = 0_i32;
    for index in (0..dot).rev() {
        match tokens[index].text.as_str() {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                if depth > 0 {
                    depth -= 1;
                    continue;
                }
                return index + 1;
            }
            _ if depth == 0 && is_expression_boundary(&tokens[index].text) => return index + 1,
            _ => {}
        }
    }
    0
}

fn is_expression_boundary(value: &str) -> bool {
    matches!(
        value,
        "=" | ":="
            | "+="
            | "-="
            | "*="
            | "/="
            | ","
            | ":"
            | ";"
            | "+"
            | "-"
            | "*"
            | "/"
            | "%"
            | "=="
            | "!="
            | "<"
            | ">"
            | "<="
            | ">="
            | "&&"
            | "||"
            | "!"
            | "?"
            | "return"
            | "if"
            | "elif"
            | "while"
            | "for"
            | "in"
            | "and"
            | "or"
            | "not"
            | "await"
    )
}

fn is_call_declaration_name(tokens: &[Token], index: usize) -> bool {
    for token in tokens[..index].iter().rev() {
        if matches!(token.text.as_str(), "func" | "signal") {
            return true;
        }
        if matches!(token.text.as_str(), ":" | "=" | ";") {
            break;
        }
    }
    false
}
