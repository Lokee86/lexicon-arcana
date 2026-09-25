use super::model::{SupertypeDecl, TokenKind};
use super::parser::Parser;
use super::parser_navigation::previous_non_newline;
use super::tokens::{
    DelimiterDepth, compact_tokens, first_top_level, identifier_text, skip_kind, split_top_level,
    trim_kind,
};

impl Parser {
    pub fn find_type_header_end(&self) -> usize {
        let mut depth = DelimiterDepth::default();
        for index in self.index..self.tokens.len() {
            let current = &self.tokens[index];
            if depth.zero() {
                if current.kind == TokenKind::Eof || matches!(current.text.as_str(), "{" | ";") {
                    return index;
                }
                if current.kind == TokenKind::Newline {
                    let next = self.next_non_newline(index + 1);
                    if next < self.tokens.len() && self.tokens[next].text == "{" {
                        return next;
                    }
                    let previous = previous_non_newline(&self.tokens, index as isize - 1);
                    if previous.is_none_or(|previous| {
                        !matches!(self.tokens[previous].text.as_str(), "," | ":")
                    }) {
                        return index;
                    }
                }
            }
            depth.update(&current.text);
        }
        self.tokens.len() - 1
    }

    pub fn parse_supertypes(&self, start: usize, mut end: usize) -> Vec<SupertypeDecl> {
        if let Some(where_token) = first_top_level(&self.tokens, start, end, "where") {
            end = where_token;
        }
        let mut supertypes = Vec::new();
        for (segment_start, segment_end) in split_top_level(&self.tokens, start, end, ",") {
            let entry_start =
                skip_kind(&self.tokens, segment_start, segment_end, TokenKind::Newline);
            let entry_end = trim_kind(&self.tokens, entry_start, segment_end, TokenKind::Newline);
            if entry_start >= entry_end {
                continue;
            }
            let by = first_top_level(&self.tokens, entry_start, entry_end, "by");
            let type_end = by.unwrap_or(entry_end);
            let delegated = by.is_some();
            supertypes.push(SupertypeDecl {
                delegate_expression: by
                    .map(|by| self.token_text(by + 1, entry_end))
                    .unwrap_or_default(),
                delegated,
                expression: self.token_text(entry_start, entry_end),
                span: self.span(entry_start, entry_end - 1),
                target_name: relationship_type_name(&self.tokens, entry_start, type_end),
            });
        }
        supertypes
    }
}

fn relationship_type_name(tokens: &[super::model::Token], start: usize, end: usize) -> String {
    let mut parts = Vec::new();
    let mut angle_depth = 0_i32;
    for current in &tokens[start..end] {
        if current.kind == TokenKind::Newline {
            continue;
        }
        match current.text.as_str() {
            "<" => {
                angle_depth += 1;
                continue;
            }
            ">" if angle_depth > 0 => {
                angle_depth -= 1;
                continue;
            }
            _ if angle_depth > 0 => continue,
            "(" => break,
            "." => parts.push(".".to_owned()),
            _ if current.kind == TokenKind::Identifier => {
                parts.push(identifier_text(current));
            }
            _ => return String::new(),
        }
    }
    let name = compact_tokens(parts.iter().map(String::as_str));
    if name.is_empty() || name.starts_with('.') || name.ends_with('.') {
        String::new()
    } else {
        name
    }
}
