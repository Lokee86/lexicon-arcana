use super::model::{SyntaxDiagnostic, Token, TokenKind};

pub fn lex(content: &[u8]) -> (Vec<Token>, Vec<SyntaxDiagnostic>) {
    Lexer::new(content).run()
}

struct Lexer<'a> {
    content: &'a [u8],
    offset: usize,
    line: u64,
    column: u64,
    tokens: Vec<Token>,
    diagnostics: Vec<SyntaxDiagnostic>,
}

#[derive(Clone, Copy)]
struct Mark {
    offset: usize,
    line: u64,
    column: u64,
}

impl<'a> Lexer<'a> {
    fn new(content: &'a [u8]) -> Self {
        Self {
            content,
            offset: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self) -> (Vec<Token>, Vec<SyntaxDiagnostic>) {
        while self.offset < self.content.len() {
            self.scan_token();
        }
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            text: String::new(),
            start_offset: self.content.len(),
            end_offset: self.content.len(),
            start_line: self.line,
            start_column: self.column,
            end_line: self.line,
            end_column: self.column,
        });
        (self.tokens, self.diagnostics)
    }

    fn scan_token(&mut self) {
        let current = self.peek_char();
        if matches!(current, '\r' | '\n') {
            self.scan_newline();
        } else if current.is_whitespace() {
            self.advance_char();
        } else if self.has_prefix("//") {
            while self.offset < self.content.len() && !matches!(self.peek_char(), '\r' | '\n') {
                self.advance_char();
            }
        } else if self.has_prefix("/*") {
            self.scan_block_comment();
        } else if self.has_prefix("\"\"\"") {
            self.scan_quoted(true);
        } else if matches!(current, '"' | '\'') {
            self.scan_quoted(false);
        } else if current == '\x60' {
            self.scan_backtick();
        } else if identifier_start(current) {
            self.scan_identifier();
        } else if current.is_ascii_digit() {
            self.scan_number();
        } else if "{}()[]<>,.:;=?@*+-/!&|%^~".contains(current) {
            let start = self.mark();
            self.advance_char();
            self.emit(TokenKind::Symbol, start);
        } else {
            let start = self.mark();
            self.advance_char();
            let token = self.token_from(TokenKind::Unknown, start);
            self.diagnostics.push(SyntaxDiagnostic {
                message: format!("unsupported character {current:?}"),
                token: token.clone(),
            });
            self.tokens.push(token);
        }
    }

    fn scan_newline(&mut self) {
        let start = self.mark();
        if self.has_prefix("\r\n") {
            self.offset += 2;
        } else {
            self.offset += 1;
        }
        self.line += 1;
        self.column = 1;
        self.emit(TokenKind::Newline, start);
    }

    fn scan_identifier(&mut self) {
        let start = self.mark();
        self.advance_char();
        while self.offset < self.content.len() && identifier_part(self.peek_char()) {
            self.advance_char();
        }
        self.emit(TokenKind::Identifier, start);
    }

    fn scan_number(&mut self) {
        let start = self.mark();
        self.advance_char();
        while self.offset < self.content.len() {
            let current = self.peek_char();
            if !current.is_ascii_alphanumeric() && current != '_' && current != '.' {
                break;
            }
            self.advance_char();
        }
        self.emit(TokenKind::Number, start);
    }

    fn scan_backtick(&mut self) {
        let start = self.mark();
        self.advance_char();
        while self.offset < self.content.len()
            && self.peek_char() != '\x60'
            && !matches!(self.peek_char(), '\r' | '\n')
        {
            self.advance_char();
        }
        if self.offset >= self.content.len() || self.peek_char() != '\x60' {
            let token = self.token_from(TokenKind::Unknown, start);
            self.diagnostics.push(SyntaxDiagnostic {
                message: "unterminated backtick identifier".into(),
                token: token.clone(),
            });
            self.tokens.push(token);
            return;
        }
        self.advance_char();
        self.emit(TokenKind::Identifier, start);
    }

    fn scan_quoted(&mut self, triple: bool) {
        let start = self.mark();
        let quote = self.peek_char();
        if triple {
            for _ in 0..3 {
                self.advance_char();
            }
            while self.offset < self.content.len() && !self.has_prefix("\"\"\"") {
                self.advance_char();
            }
            if !self.has_prefix("\"\"\"") {
                self.push_diagnostic(start, "unterminated triple-quoted string");
                return;
            }
            for _ in 0..3 {
                self.advance_char();
            }
            self.emit(TokenKind::String, start);
            return;
        }

        self.advance_char();
        let mut escaped = false;
        while self.offset < self.content.len() {
            let current = self.peek_char();
            if matches!(current, '\r' | '\n') {
                break;
            }
            self.advance_char();
            if current == quote && !escaped {
                self.emit(TokenKind::String, start);
                return;
            }
            escaped = current == '\\' && !escaped;
        }
        self.push_diagnostic(start, "unterminated quoted literal");
    }

    fn scan_block_comment(&mut self) {
        let start = self.mark();
        self.advance_char();
        self.advance_char();
        let mut depth = 1;
        while self.offset < self.content.len() && depth > 0 {
            if self.has_prefix("/*") {
                self.advance_char();
                self.advance_char();
                depth += 1;
            } else if self.has_prefix("*/") {
                self.advance_char();
                self.advance_char();
                depth -= 1;
            } else {
                self.advance_char();
            }
        }
        if depth != 0 {
            self.push_diagnostic(start, "unterminated block comment");
        }
    }

    fn push_diagnostic(&mut self, start: Mark, message: &str) {
        let token = self.token_from(TokenKind::Unknown, start);
        self.diagnostics.push(SyntaxDiagnostic {
            message: message.into(),
            token: token.clone(),
        });
        self.tokens.push(token);
    }

    fn mark(&self) -> Mark {
        Mark {
            offset: self.offset,
            line: self.line,
            column: self.column,
        }
    }

    fn emit(&mut self, kind: TokenKind, start: Mark) {
        let token = self.token_from(kind, start);
        self.tokens.push(token);
    }

    fn token_from(&self, kind: TokenKind, start: Mark) -> Token {
        Token {
            kind,
            text: String::from_utf8_lossy(&self.content[start.offset..self.offset]).into_owned(),
            start_offset: start.offset,
            end_offset: self.offset,
            start_line: start.line,
            start_column: start.column,
            end_line: self.line,
            end_column: self.column,
        }
    }

    fn advance_char(&mut self) -> char {
        if self.offset >= self.content.len() {
            return '\0';
        }
        if self.has_prefix("\r\n") {
            self.offset += 2;
            self.line += 1;
            self.column = 1;
            return '\n';
        }
        let value = std::str::from_utf8(&self.content[self.offset..])
            .ok()
            .and_then(|value| value.chars().next())
            .unwrap_or('\u{FFFD}');
        self.offset += value.len_utf8().max(1);
        if matches!(value, '\r' | '\n') {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        value
    }

    fn peek_char(&self) -> char {
        std::str::from_utf8(&self.content[self.offset..])
            .ok()
            .and_then(|value| value.chars().next())
            .unwrap_or('\u{FFFD}')
    }

    fn has_prefix(&self, value: &str) -> bool {
        self.content[self.offset..].starts_with(value.as_bytes())
    }
}

fn identifier_start(value: char) -> bool {
    value == '_' || value.is_alphabetic()
}

fn identifier_part(value: char) -> bool {
    identifier_start(value) || value.is_ascii_digit()
}
