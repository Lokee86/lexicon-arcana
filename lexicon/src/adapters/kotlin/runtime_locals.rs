use super::model::{Token, TokenKind, TokenRange};
use super::runtime::RuntimeCallable;
use super::runtime_tokens::{matching_delimiter, nested_ranges, next_token};
use super::tokens::{DelimiterDepth, compact_range};

#[derive(Default)]
struct RuntimeLocalType {
    name: String,
    scope_end: usize,
    scope_start: usize,
    spelling: String,
    visible_at: usize,
}

pub fn declared_type(callable: &RuntimeCallable, name: &str, call_at: usize) -> Option<String> {
    let bounds = &callable.declaration.body;
    if call_at < bounds.start || call_at >= bounds.end {
        return None;
    }
    let tokens = &callable.file.tokens;
    let ignored = nested_ranges(tokens, bounds);
    let mut best: Option<RuntimeLocalType> = None;
    let mut index = bounds.start;
    while index < call_at {
        if let Some(nested) = ignored.iter().find(|range| range.start == index) {
            index = nested.end;
            continue;
        }
        if !matches!(tokens[index].text.as_str(), "val" | "var") {
            index += 1;
            continue;
        }
        let (local, next) = parse_local(tokens, bounds, index);
        index = next.max(index + 1);
        if local.name != name || local.visible_at > call_at || call_at >= local.scope_end {
            continue;
        }
        if best.as_ref().is_none_or(|current| {
            local.scope_start > current.scope_start
                || (local.scope_start == current.scope_start
                    && local.visible_at > current.visible_at)
        }) {
            best = Some(local);
        }
    }
    best.map(|value| value.spelling)
}

fn parse_local(tokens: &[Token], bounds: &TokenRange, start: usize) -> (RuntimeLocalType, usize) {
    let mut local = RuntimeLocalType::default();
    let name = next_token(tokens, start + 1, bounds.end);
    let statement_end = statement_end(tokens, name, bounds.end);
    if name >= bounds.end || tokens[name].kind != TokenKind::Identifier {
        return (local, statement_end);
    }
    local.name = super::tokens::identifier_text(&tokens[name]);
    local.visible_at = statement_end;
    (local.scope_start, local.scope_end) = local_scope(tokens, bounds, start);
    let colon = next_token(tokens, name + 1, statement_end);
    if colon >= statement_end || tokens[colon].text != ":" {
        return (local, statement_end);
    }
    let type_start = next_token(tokens, colon + 1, statement_end);
    let mut type_end = type_start;
    let mut depth = DelimiterDepth::default();
    while type_end < statement_end {
        let current = &tokens[type_end];
        if depth.zero()
            && (matches!(current.text.as_str(), "=" | "by") || current.kind == TokenKind::Newline)
        {
            break;
        }
        depth.update(&current.text);
        type_end += 1;
    }
    local.spelling = compact_range(tokens, type_start, type_end);
    (local, statement_end)
}

fn statement_end(tokens: &[Token], start: usize, upper: usize) -> usize {
    let mut depth = DelimiterDepth::default();
    for (index, current) in tokens.iter().enumerate().take(upper).skip(start) {
        if depth.zero() && (current.kind == TokenKind::Newline || current.text == ";") {
            return index + 1;
        }
        depth.update(&current.text);
    }
    upper
}

fn local_scope(tokens: &[Token], bounds: &TokenRange, declaration: usize) -> (usize, usize) {
    let mut start = bounds.start;
    let mut end = bounds.end;
    for index in bounds.start..declaration {
        if tokens[index].text != "{" {
            continue;
        }
        if let Some(close) = matching_delimiter(tokens, index, bounds.end, "{", "}")
            && close > declaration
            && index >= start
            && close <= end
        {
            start = index + 1;
            end = close;
        }
    }
    (start, end)
}
