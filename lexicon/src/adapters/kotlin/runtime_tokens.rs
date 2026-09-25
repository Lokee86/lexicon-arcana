use crate::SourceSpan;

use super::model::{ParsedFile, Token, TokenKind, TokenRange};
use super::tokens::{
    compact_range, identifier_text, last_token, skip_kind, split_top_level, trim_kind,
};

#[derive(Debug, Clone)]
pub struct RuntimeInvocation {
    pub arity: usize,
    pub callee_start: usize,
    pub expression: String,
    pub fluent: bool,
    pub name: String,
    pub qualifier: String,
    pub span: SourceSpan,
    pub unsupported: bool,
}

pub fn invocations(file: &ParsedFile, ranges: &[TokenRange]) -> Vec<RuntimeInvocation> {
    let mut result = Vec::new();
    for bounds in ranges {
        if bounds.end <= bounds.start {
            continue;
        }
        let ignored = nested_ranges(&file.tokens, bounds);
        for index in bounds.start..bounds.end.min(file.tokens.len()) {
            if token_ignored(index, &ignored) || file.tokens[index].kind != TokenKind::Identifier {
                continue;
            }
            let open = next_token(&file.tokens, index + 1, bounds.end);
            if open >= bounds.end
                || file.tokens[open].text != "("
                || control_call(&file.tokens[index].text)
            {
                continue;
            }
            let Some(close) = matching_delimiter(&file.tokens, open, bounds.end, "(", ")") else {
                continue;
            };
            let (start, unsupported) = callee_start(&file.tokens, index, bounds.start);
            let previous = previous_token(&file.tokens, start as isize - 1, bounds.start);
            if previous
                .is_some_and(|previous| matches!(file.tokens[previous].text.as_str(), "fun" | "@"))
            {
                continue;
            }
            let qualifier = last_token(&file.tokens, start, index, ".")
                .map(|dot| compact_range(&file.tokens, start, dot))
                .unwrap_or_default();
            result.push(RuntimeInvocation {
                arity: argument_arity(&file.tokens, open + 1, close),
                callee_start: start,
                expression: compact_range(&file.tokens, start, close + 1),
                fluent: call_result_is_receiver(&file.tokens, close, bounds.end),
                name: identifier_text(&file.tokens[index]),
                qualifier,
                span: token_span(&file.path, &file.tokens[start], &file.tokens[close]),
                unsupported,
            });
        }
    }
    result
}

pub fn call_result_is_receiver(tokens: &[Token], close: usize, upper: usize) -> bool {
    let next = next_token(tokens, close + 1, upper);
    if next >= upper {
        return false;
    }
    if tokens[next].text == "." {
        return true;
    }
    if tokens[next].text != "?" {
        return false;
    }
    let dot = next_token(tokens, next + 1, upper);
    dot < upper && tokens[dot].text == "."
}

pub fn callee_start(tokens: &[Token], name: usize, lower: usize) -> (usize, bool) {
    let mut start = name;
    let mut unsupported = false;
    while let Some(dot) = previous_token(tokens, start as isize - 1, lower) {
        if tokens[dot].text != "." {
            break;
        }
        let Some(owner) = previous_token(tokens, dot as isize - 1, lower) else {
            unsupported = true;
            break;
        };
        if tokens[owner].kind != TokenKind::Identifier {
            unsupported = true;
            break;
        }
        if previous_token(tokens, owner as isize - 1, lower)
            .is_some_and(|question| tokens[question].text == "?")
        {
            unsupported = true;
            break;
        }
        start = owner;
    }
    if previous_token(tokens, start as isize - 1, lower)
        .is_some_and(|previous| matches!(tokens[previous].text.as_str(), "." | "?"))
    {
        unsupported = true;
    }
    (start, unsupported)
}

pub fn argument_arity(tokens: &[Token], start: usize, end: usize) -> usize {
    split_top_level(tokens, start, end, ",")
        .into_iter()
        .filter(|(left, right)| {
            let left = skip_kind(tokens, *left, *right, TokenKind::Newline);
            let right = trim_kind(tokens, left, *right, TokenKind::Newline);
            left < right
        })
        .count()
}

pub fn next_token(tokens: &[Token], mut index: usize, upper: usize) -> usize {
    while index < upper && tokens[index].kind == TokenKind::Newline {
        index += 1;
    }
    index
}

pub fn previous_token(tokens: &[Token], mut index: isize, lower: usize) -> Option<usize> {
    while index >= lower as isize && tokens[index as usize].kind == TokenKind::Newline {
        index -= 1;
    }
    (index >= lower as isize).then_some(index as usize)
}

pub fn matching_delimiter(
    tokens: &[Token],
    open: usize,
    upper: usize,
    opening: &str,
    closing: &str,
) -> Option<usize> {
    let mut depth = 0_i32;
    for (index, token) in tokens.iter().enumerate().take(upper).skip(open) {
        match token.text.as_str() {
            value if value == opening => depth += 1,
            value if value == closing => {
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

pub fn token_span(path: &str, first: &Token, last: &Token) -> SourceSpan {
    SourceSpan {
        end_column: last.end_column,
        end_line: last.end_line,
        path: path.into(),
        start_column: first.start_column,
        start_line: first.start_line,
    }
}

pub fn nested_ranges(tokens: &[Token], bounds: &TokenRange) -> Vec<TokenRange> {
    let mut result = Vec::new();
    let mut index = bounds.start;
    while index < bounds.end.min(tokens.len()) {
        if matches!(tokens[index].text.as_str(), "fun" | "class" | "object") {
            let nested = nested_declaration_range(tokens, index, bounds.end);
            if nested.end > nested.start {
                index = nested.end;
                result.push(nested);
                continue;
            }
        }
        if tokens[index].text == "{"
            && lambda_brace(tokens, index, bounds.start, bounds.end)
            && let Some(close) = matching_delimiter(tokens, index, bounds.end, "{", "}")
        {
            result.push(TokenRange {
                start: index,
                end: close + 1,
            });
            index = close + 1;
            continue;
        }
        index += 1;
    }
    result
}

fn nested_declaration_range(tokens: &[Token], start: usize, upper: usize) -> TokenRange {
    for index in start + 1..upper {
        if tokens[index].text == "{" {
            return matching_delimiter(tokens, index, upper, "{", "}")
                .map(|close| TokenRange {
                    start,
                    end: close + 1,
                })
                .unwrap_or_default();
        }
        if tokens[index].text == "=" {
            let mut end = index + 1;
            while end < upper && tokens[end].kind != TokenKind::Newline && tokens[end].text != ";" {
                end += 1;
            }
            return TokenRange { start, end };
        }
    }
    TokenRange::default()
}

fn lambda_brace(tokens: &[Token], open: usize, lower: usize, upper: usize) -> bool {
    if let Some(close) = matching_delimiter(tokens, open, upper, "{", "}") {
        for index in open + 1..close {
            if tokens[index].text == "-" {
                let next = next_token(tokens, index + 1, close);
                if next < close && tokens[next].text == ">" {
                    return true;
                }
            }
        }
    }
    let Some(previous) = previous_token(tokens, open as isize - 1, lower) else {
        return false;
    };
    if tokens[previous].text == "=" {
        return true;
    }
    tokens[previous].kind == TokenKind::Identifier
        && !matches!(
            tokens[previous].text.as_str(),
            "do" | "else" | "finally" | "try" | "when"
        )
}

pub fn token_ignored(index: usize, ranges: &[TokenRange]) -> bool {
    ranges
        .iter()
        .any(|bounds| index >= bounds.start && index < bounds.end)
}

fn control_call(name: &str) -> bool {
    matches!(
        name,
        "catch" | "for" | "if" | "val" | "var" | "when" | "while"
    )
}
