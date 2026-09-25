use std::collections::BTreeSet;

use crate::SourceSpan;

use super::lexer::lex;
use super::model::{Declaration, ImportDecl, ParsedFile, SyntaxDiagnostic, Token, TokenKind};
use super::tokens::{compact_tokens, identifier_text, unique_sorted};

const DECLARATION_MODIFIERS: &[&str] = &[
    "abstract",
    "actual",
    "annotation",
    "companion",
    "const",
    "crossinline",
    "data",
    "enum",
    "expect",
    "external",
    "final",
    "infix",
    "inline",
    "inner",
    "internal",
    "lateinit",
    "noinline",
    "open",
    "operator",
    "out",
    "override",
    "private",
    "protected",
    "public",
    "reified",
    "sealed",
    "suspend",
    "tailrec",
    "value",
    "vararg",
];

#[derive(Default)]
pub struct DeclarationPrefix {
    pub annotations: Vec<String>,
    pub modifiers: Vec<String>,
    pub start: usize,
}

pub struct Parser {
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub index: usize,
    pub path: String,
    pub tokens: Vec<Token>,
}

pub fn parse_file(path: &str, content: &[u8]) -> ParsedFile {
    let (tokens, diagnostics) = lex(content);
    let mut state = Parser {
        diagnostics,
        index: 0,
        path: path.into(),
        tokens,
    };
    let mut result = ParsedFile {
        content: content.to_vec(),
        declarations: Vec::new(),
        diagnostics: Vec::new(),
        imports: Vec::new(),
        package_name: String::new(),
        package_span: None,
        path: path.into(),
        tokens: Vec::new(),
    };

    state.skip_separators();
    while state.at("@") {
        state.parse_annotation();
        state.skip_separators();
    }
    if state.at("package") {
        let start = state.index;
        state.index += 1;
        if let Some((name, end)) = state.parse_qualified_directive(false) {
            result.package_name = name;
            result.package_span = Some(state.span(start, end));
        } else {
            state.add_diagnostic(start, "malformed package directive");
        }
    }
    state.skip_separators();

    while state.at("import") {
        let start = state.index;
        state.index += 1;
        let mut parsed = state.parse_qualified_directive(true);
        let mut alias = String::new();
        if state.at("as") {
            state.index += 1;
            if state.current().kind == TokenKind::Identifier {
                alias = identifier_text(&state.current());
                state.index += 1;
            } else {
                parsed = None;
            }
        }
        if let Some((name, end)) = parsed {
            result.imports.push(ImportDecl {
                alias,
                wildcard: name.ends_with(".*"),
                path: name,
                span: state.span(start, end),
            });
        } else {
            state.add_diagnostic(start, "malformed import directive");
        }
        state.skip_to_line_end();
        state.skip_separators();
    }

    result.declarations = state.parse_scope(false);
    result.diagnostics = state.diagnostics;
    result.tokens = state.tokens;
    result
}

impl Parser {
    pub fn parse_scope(&mut self, stop_at_brace: bool) -> Vec<Declaration> {
        let mut declarations = Vec::new();
        while self.current().kind != TokenKind::Eof {
            self.skip_separators();
            if stop_at_brace && self.at("}") {
                self.index += 1;
                return declarations;
            }
            if self.current().kind == TokenKind::Eof {
                break;
            }
            if matches!(self.current().text.as_str(), ")" | "]" | "}") {
                self.add_diagnostic(self.index, "unmatched closing delimiter");
                self.index += 1;
                continue;
            }

            let prefix = self.parse_prefix();
            let declaration = match self.current().text.as_str() {
                "class" | "interface" | "object" => self.parse_type(prefix),
                "fun" => self.parse_function(prefix),
                "val" | "var" => self.parse_property(prefix),
                "constructor" => self.parse_constructor(prefix, false),
                _ => {
                    if !prefix.annotations.is_empty() || !prefix.modifiers.is_empty() {
                        self.add_diagnostic(prefix.start, "unsupported or malformed declaration");
                    }
                    self.skip_statement();
                    None
                }
            };
            if let Some(declaration) = declaration {
                declarations.push(declaration);
            }
        }
        if stop_at_brace {
            self.add_diagnostic(self.index.saturating_sub(1), "unclosed declaration body");
        }
        declarations
    }

    fn parse_prefix(&mut self) -> DeclarationPrefix {
        let mut prefix = DeclarationPrefix {
            start: self.index,
            ..Default::default()
        };
        let modifiers = BTreeSet::from_iter(DECLARATION_MODIFIERS.iter().copied());
        loop {
            if self.at("@") {
                if let Some(annotation) = self.parse_annotation() {
                    prefix.annotations.push(annotation);
                }
                self.skip_newlines();
                continue;
            }
            if modifiers.contains(self.current().text.as_str()) {
                prefix.modifiers.push(self.current().text.clone());
                self.index += 1;
                self.skip_newlines();
                continue;
            }
            break;
        }
        prefix.annotations = unique_sorted(prefix.annotations);
        prefix.modifiers = unique_sorted(prefix.modifiers);
        prefix
    }

    pub(crate) fn parse_annotation(&mut self) -> Option<String> {
        if !self.at("@") {
            return None;
        }
        self.index += 1;
        let mut parts = Vec::<String>::new();
        while self.current().kind == TokenKind::Identifier
            || matches!(self.current().text.as_str(), "." | ":")
        {
            parts.push(self.current().text.clone());
            self.index += 1;
        }
        if self.at("[") {
            self.skip_balanced("[", "]");
        } else if self.at("(") {
            self.skip_balanced("(", ")");
        }
        let compact = compact_tokens(parts.iter().map(String::as_str));
        (!compact.is_empty()).then_some(compact)
    }

    pub fn span(&self, start: usize, end: usize) -> SourceSpan {
        let start = start.min(self.tokens.len().saturating_sub(1));
        let end = end.max(start).min(self.tokens.len().saturating_sub(1));
        let first = &self.tokens[start];
        let last = &self.tokens[end];
        SourceSpan {
            end_column: last.end_column,
            end_line: last.end_line,
            path: self.path.clone(),
            start_column: first.start_column,
            start_line: first.start_line,
        }
    }

    pub fn add_diagnostic(&mut self, index: usize, message: &str) {
        if self.tokens.is_empty() {
            return;
        }
        let index = index.min(self.tokens.len() - 1);
        self.diagnostics.push(SyntaxDiagnostic {
            message: message.into(),
            token: self.tokens[index].clone(),
        });
    }
}
