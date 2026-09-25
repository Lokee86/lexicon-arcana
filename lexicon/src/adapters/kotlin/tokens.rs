use std::collections::BTreeSet;

use super::model::{Token, TokenKind};

#[derive(Default, Clone, Copy)]
pub struct DelimiterDepth {
    braces: i32,
    brackets: i32,
    parentheses: i32,
    angles: i32,
}

impl DelimiterDepth {
    pub fn update(&mut self, text: &str) {
        match text {
            "{" => self.braces += 1,
            "}" if self.braces > 0 => self.braces -= 1,
            "[" => self.brackets += 1,
            "]" if self.brackets > 0 => self.brackets -= 1,
            "(" => self.parentheses += 1,
            ")" if self.parentheses > 0 => self.parentheses -= 1,
            "<" => self.angles += 1,
            ">" if self.angles > 0 => self.angles -= 1,
            _ => {}
        }
    }

    pub fn zero(self) -> bool {
        self.braces == 0 && self.brackets == 0 && self.parentheses == 0 && self.angles == 0
    }
}

pub fn split_top_level(
    tokens: &[Token],
    start: usize,
    end: usize,
    separator: &str,
) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut segment_start = start;
    let mut depth = DelimiterDepth::default();
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        if depth.zero() && token.text == separator {
            result.push((segment_start, index));
            segment_start = index + 1;
            continue;
        }
        depth.update(&token.text);
    }
    result.push((segment_start, end));
    result
}

pub fn first_top_level(tokens: &[Token], start: usize, end: usize, wanted: &str) -> Option<usize> {
    let mut depth = DelimiterDepth::default();
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        if depth.zero() && token.text == wanted {
            return Some(index);
        }
        depth.update(&token.text);
    }
    None
}

pub fn last_identifier(tokens: &[Token], start: usize, end: usize) -> Option<usize> {
    (start..end)
        .rev()
        .find(|index| tokens[*index].kind == TokenKind::Identifier)
}

pub fn last_token(tokens: &[Token], start: usize, end: usize, wanted: &str) -> Option<usize> {
    (start..end)
        .rev()
        .find(|index| tokens[*index].text == wanted)
}

pub fn compact_range(tokens: &[Token], start: usize, end: usize) -> String {
    compact_tokens(
        tokens[start.min(tokens.len())..end.min(tokens.len())]
            .iter()
            .filter(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
            .map(|token| token.text.as_str()),
    )
}

pub fn compact_tokens<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut output = String::new();
    let mut previous = "";
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if !output.is_empty() && needs_space(previous, part) {
            output.push(' ');
        }
        output.push_str(part);
        previous = part;
    }
    output.trim().to_owned()
}

fn needs_space(previous: &str, current: &str) -> bool {
    if previous.is_empty() {
        return false;
    }
    if ").,?]>:;".contains(current)
        || "([<.@:".contains(previous)
        || matches!(current, "<" | "(" | "[" | "." | "@")
        || matches!(previous, "." | "?" | "!" | "~")
    {
        return false;
    }
    true
}

pub fn annotation_end(tokens: &[Token], start: usize, end: usize) -> usize {
    let mut index = start + 1;
    while index < end
        && (tokens[index].kind == TokenKind::Identifier
            || matches!(tokens[index].text.as_str(), "." | ":"))
    {
        index += 1;
    }
    if index < end && tokens[index].text == "(" {
        let mut depth = 0_i32;
        while index < end {
            match tokens[index].text.as_str() {
                "(" => depth += 1,
                ")" => {
                    depth -= 1;
                    if depth == 0 {
                        return index + 1;
                    }
                }
                _ => {}
            }
            index += 1;
        }
    }
    index
}

pub fn skip_kind(tokens: &[Token], mut start: usize, end: usize, kind: TokenKind) -> usize {
    while start < end && tokens[start].kind == kind {
        start += 1;
    }
    start
}

pub fn trim_kind(tokens: &[Token], start: usize, mut end: usize, kind: TokenKind) -> usize {
    while end > start && tokens[end - 1].kind == kind {
        end -= 1;
    }
    end
}

pub fn identifier_text(token: &Token) -> String {
    token
        .text
        .strip_prefix('\x60')
        .and_then(|value| value.strip_suffix('\x60'))
        .unwrap_or(&token.text)
        .to_owned()
}

pub fn unique_sorted(values: impl IntoIterator<Item = String>) -> Vec<String> {
    BTreeSet::from_iter(values.into_iter().filter(|value| !value.is_empty()))
        .into_iter()
        .collect()
}

pub fn contains(values: &[String], wanted: &str) -> bool {
    values.iter().any(|value| value == wanted)
}

pub fn nullable_type(value: &str) -> bool {
    value.trim().ends_with('?')
}
