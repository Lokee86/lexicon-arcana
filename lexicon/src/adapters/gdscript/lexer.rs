use crate::{AdapterError, SourceSpan};

use super::model::{Statement, Token, TokenKind};

pub fn lex(source: &str) -> Result<Vec<Token>, AdapterError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let (mut line, mut column) = (1_u64, 1_u64);
    let mut index = 0_usize;

    while index < bytes.len() {
        let current = bytes[index];
        if current == b'\r' {
            index += 1;
            continue;
        }
        if current == b'\n' {
            tokens.push(Token {
                kind: TokenKind::Symbol,
                text: "\n".into(),
                line,
                column,
                end_line: line,
                end_column: column + 1,
            });
            index += 1;
            line += 1;
            column = 1;
            continue;
        }
        if matches!(current, b' ' | b'\t') {
            index += 1;
            column += if current == b'\t' { 4 } else { 1 };
            continue;
        }
        if current == b'#' {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
                column += 1;
            }
            continue;
        }

        let (start_line, start_column, start) = (line, column, index);
        if matches!(current, b'\'' | b'"') {
            let quote = current;
            let triple =
                index + 2 < bytes.len() && bytes[index + 1] == quote && bytes[index + 2] == quote;
            if triple {
                index += 3;
                column += 3;
            } else {
                index += 1;
                column += 1;
            }
            let mut closed = false;
            while index < bytes.len() {
                if triple
                    && index + 2 < bytes.len()
                    && bytes[index] == quote
                    && bytes[index + 1] == quote
                    && bytes[index + 2] == quote
                {
                    index += 3;
                    column += 3;
                    closed = true;
                    break;
                }
                if !triple && bytes[index] == quote {
                    index += 1;
                    column += 1;
                    closed = true;
                    break;
                }
                if bytes[index] == b'\\' && index + 1 < bytes.len() {
                    index += 2;
                    column += 2;
                    continue;
                }
                if bytes[index] == b'\n' {
                    index += 1;
                    line += 1;
                    column = 1;
                    continue;
                }
                index += 1;
                column += 1;
            }
            if !closed {
                return Err(AdapterError::new(format!(
                    "unterminated string at {start_line}:{start_column}"
                )));
            }
            tokens.push(Token {
                kind: TokenKind::String,
                text: source[start..index].to_owned(),
                line: start_line,
                column: start_column,
                end_line: line,
                end_column: column,
            });
            continue;
        }

        if is_identifier_start(current) {
            index += 1;
            column += 1;
            while index < bytes.len() && is_identifier_part(bytes[index]) {
                index += 1;
                column += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Identifier,
                text: source[start..index].to_owned(),
                line: start_line,
                column: start_column,
                end_line: line,
                end_column: column,
            });
            continue;
        }

        if current.is_ascii_digit() {
            index += 1;
            column += 1;
            while index < bytes.len() && (is_identifier_part(bytes[index]) || bytes[index] == b'.')
            {
                index += 1;
                column += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Number,
                text: source[start..index].to_owned(),
                line: start_line,
                column: start_column,
                end_line: line,
                end_column: column,
            });
            continue;
        }

        let mut width = 1;
        if index + 1 < bytes.len() {
            let pair = &source[index..index + 2];
            if matches!(
                pair,
                "->" | ":="
                    | "=="
                    | "!="
                    | "<="
                    | ">="
                    | "&&"
                    | "||"
                    | "+="
                    | "-="
                    | "*="
                    | "/="
                    | "++"
                    | "--"
            ) {
                width = 2;
            }
        }
        let symbol = source[index..index + width].to_owned();
        index += width;
        column += width as u64;
        tokens.push(Token {
            kind: TokenKind::Symbol,
            text: symbol,
            line: start_line,
            column: start_column,
            end_line: line,
            end_column: column,
        });
    }

    Ok(tokens)
}

pub fn make_statements(tokens: &[Token]) -> Vec<Statement> {
    let mut statements = Vec::new();
    let mut current = Vec::new();
    let mut depth = 0_i32;

    for token in tokens {
        if token.text == "\n" {
            if depth == 0 && !current.is_empty() {
                push_statement(&mut statements, std::mem::take(&mut current));
            }
            continue;
        }
        current.push(token.clone());
        match token.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth > 0 => depth -= 1,
            _ => {}
        }
    }
    if !current.is_empty() {
        push_statement(&mut statements, current);
    }
    statements
}

pub fn span(path: &str, start: &Token, end: &Token) -> SourceSpan {
    SourceSpan {
        path: path.into(),
        start_line: start.line,
        start_column: start.column,
        end_line: end.end_line,
        end_column: end.end_column,
    }
}

fn push_statement(statements: &mut Vec<Statement>, tokens: Vec<Token>) {
    let start = tokens.first().expect("nonempty statement").clone();
    let end = tokens.last().expect("nonempty statement").clone();
    statements.push(Statement {
        indent: start.column.saturating_sub(1) as usize,
        start,
        end,
        tokens,
    });
}

fn is_identifier_start(value: u8) -> bool {
    value == b'_' || value.is_ascii_alphabetic()
}

fn is_identifier_part(value: u8) -> bool {
    is_identifier_start(value) || value.is_ascii_digit()
}
