use super::model::{Token, TokenKind};
use super::parser::Parser;
use super::tokens::{DelimiterDepth, compact_range};

impl Parser {
    pub fn parse_qualified_directive(&mut self, allow_wildcard: bool) -> Option<(String, usize)> {
        let mut parts = Vec::new();
        let mut last = self.index;
        let mut expect_name = true;
        while self.current().kind != TokenKind::Eof
            && self.current().kind != TokenKind::Newline
            && !self.at(";")
            && !self.at("as")
        {
            let current = self.current();
            if expect_name {
                if current.kind == TokenKind::Identifier || (allow_wildcard && current.text == "*")
                {
                    parts.push(super::tokens::identifier_text(&current));
                    last = self.index;
                    self.index += 1;
                    expect_name = false;
                    continue;
                }
                return None;
            }
            if current.text != "." {
                break;
            }
            parts.push(".".into());
            last = self.index;
            self.index += 1;
            expect_name = true;
        }
        if parts.is_empty() || expect_name {
            return None;
        }
        Some((parts.concat(), last))
    }

    pub fn skip_balanced(&mut self, open: &str, close: &str) -> bool {
        if !self.at(open) {
            return false;
        }
        let Some(matching) = self.matching_delimiter(self.index, open, close) else {
            self.add_diagnostic(self.index, &format!("unclosed {open} delimiter"));
            self.index = self.find_recovery_boundary(self.index + 1);
            return false;
        };
        self.index = matching + 1;
        true
    }

    pub fn matching_delimiter(&self, open: usize, opening: &str, closing: &str) -> Option<usize> {
        let mut depth = 0_i32;
        for index in open..self.tokens.len() {
            match self.tokens[index].text.as_str() {
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

    pub fn find_header_token(&self, wanted: &str) -> Option<usize> {
        let mut angle = 0_i32;
        for index in self.index..self.tokens.len() {
            let current = &self.tokens[index];
            if current.kind == TokenKind::Eof
                || matches!(current.text.as_str(), "{" | "=" | ";")
                || (current.kind == TokenKind::Newline && angle == 0)
            {
                return None;
            }
            match current.text.as_str() {
                "<" => angle += 1,
                ">" if angle > 0 => angle -= 1,
                value if value == wanted && angle == 0 => return Some(index),
                _ => {}
            }
        }
        None
    }

    pub fn find_type_end(&self) -> usize {
        let mut depth = DelimiterDepth::default();
        for index in self.index..self.tokens.len() {
            let current = &self.tokens[index];
            if depth.zero()
                && (matches!(current.kind, TokenKind::Eof | TokenKind::Newline)
                    || matches!(current.text.as_str(), "{" | "=" | ";" | "where"))
            {
                return index;
            }
            depth.update(&current.text);
        }
        self.tokens.len() - 1
    }

    pub fn find_property_header_end(&self) -> usize {
        let mut depth = DelimiterDepth::default();
        for index in self.index..self.tokens.len() {
            let current = &self.tokens[index];
            if depth.zero()
                && (matches!(current.kind, TokenKind::Eof | TokenKind::Newline)
                    || matches!(current.text.as_str(), ";" | "{"))
            {
                return index;
            }
            depth.update(&current.text);
        }
        self.tokens.len() - 1
    }

    pub fn skip_expression(&mut self) {
        let mut depth = DelimiterDepth::default();
        while self.current().kind != TokenKind::Eof {
            let current = self.current();
            if depth.zero()
                && (current.kind == TokenKind::Newline
                    || matches!(current.text.as_str(), ";" | "}"))
            {
                return;
            }
            depth.update(&current.text);
            self.index += 1;
        }
    }

    pub fn skip_statement(&mut self) {
        let mut depth = DelimiterDepth::default();
        while self.current().kind != TokenKind::Eof {
            let current = self.current();
            if depth.zero()
                && (current.kind == TokenKind::Newline
                    || matches!(current.text.as_str(), ";" | "}"))
            {
                if current.kind == TokenKind::Newline || current.text == ";" {
                    self.index += 1;
                }
                return;
            }
            depth.update(&current.text);
            self.index += 1;
        }
    }

    pub fn skip_to_line_end(&mut self) {
        while self.current().kind != TokenKind::Eof
            && self.current().kind != TokenKind::Newline
            && !self.at(";")
        {
            self.index += 1;
        }
    }

    pub fn find_recovery_boundary(&self, start: usize) -> usize {
        (start..self.tokens.len())
            .find(|index| {
                self.tokens[*index].kind == TokenKind::Newline
                    || self.tokens[*index].kind == TokenKind::Eof
                    || matches!(self.tokens[*index].text.as_str(), ";" | "}")
            })
            .unwrap_or(self.tokens.len() - 1)
    }

    pub fn skip_separators(&mut self) {
        while self.current().kind == TokenKind::Newline || self.at(";") {
            self.index += 1;
        }
    }

    pub fn skip_newlines(&mut self) {
        while self.current().kind == TokenKind::Newline {
            self.index += 1;
        }
    }

    pub fn current(&self) -> Token {
        self.tokens
            .get(self.index)
            .cloned()
            .unwrap_or_else(|| self.tokens.last().cloned().expect("EOF token"))
    }

    pub fn at(&self, text: &str) -> bool {
        self.current().text == text
    }

    pub fn next_non_newline(&self, mut index: usize) -> usize {
        while index < self.tokens.len() && self.tokens[index].kind == TokenKind::Newline {
            index += 1;
        }
        index
    }

    pub fn token_text(&self, start: usize, end: usize) -> String {
        compact_range(&self.tokens, start, end)
    }
}

pub fn previous_non_newline(tokens: &[Token], mut index: isize) -> Option<usize> {
    while index >= 0 && tokens[index as usize].kind == TokenKind::Newline {
        index -= 1;
    }
    (index >= 0).then_some(index as usize)
}
