use super::model::{Declaration, TokenKind, TokenRange};
use super::parser::{DeclarationPrefix, Parser};
use super::tokens::{contains, first_top_level, identifier_text, last_identifier, last_token};

impl Parser {
    pub fn parse_type(&mut self, prefix: DeclarationPrefix) -> Option<Declaration> {
        let keyword_index = self.index;
        let keyword = self.current().text;
        self.index += 1;

        let name = if keyword == "object" && contains(&prefix.modifiers, "companion") {
            let mut name = "Companion".to_owned();
            if self.current().kind == TokenKind::Identifier && !self.at(":") {
                name = identifier_text(&self.current());
                self.index += 1;
            }
            name
        } else if self.current().kind == TokenKind::Identifier {
            let name = identifier_text(&self.current());
            self.index += 1;
            name
        } else {
            self.add_diagnostic(keyword_index, "type declaration is missing a name");
            self.skip_statement();
            return None;
        };

        if self.at("<") {
            self.skip_balanced("<", ">");
        }
        while self.at("@") || is_visibility(&self.current().text) {
            if self.at("@") {
                self.parse_annotation();
            } else {
                self.index += 1;
            }
            self.skip_newlines();
        }

        let mut explicit_constructor = false;
        if self.at("constructor") {
            explicit_constructor = true;
            self.index += 1;
            self.skip_newlines();
        }
        let mut parameters = Vec::new();
        if self.at("(") {
            parameters = self.parse_parameter_list()?;
            explicit_constructor = true;
        }

        let kind = if keyword == "interface" {
            "interface"
        } else {
            "type"
        };
        let form = if keyword == "object" && contains(&prefix.modifiers, "companion") {
            "companion_object"
        } else if keyword == "object" && contains(&prefix.modifiers, "data") {
            "data_object"
        } else if keyword == "object" {
            "object"
        } else if contains(&prefix.modifiers, "enum") {
            "enum_class"
        } else if contains(&prefix.modifiers, "data") {
            "data_class"
        } else if contains(&prefix.modifiers, "value") {
            "value_class"
        } else if contains(&prefix.modifiers, "annotation") {
            "annotation_class"
        } else if contains(&prefix.modifiers, "sealed") && keyword == "interface" {
            "sealed_interface"
        } else if contains(&prefix.modifiers, "sealed") {
            "sealed_class"
        } else {
            keyword.as_str()
        };

        let mut declaration = Declaration {
            annotations: prefix.annotations,
            body: TokenRange::default(),
            children: Vec::new(),
            delegation: TokenRange::default(),
            delegated: false,
            form: form.into(),
            kind: kind.into(),
            modifiers: prefix.modifiers,
            mutable: false,
            name: name.clone(),
            parameters: parameters.clone(),
            primary: false,
            receiver: String::new(),
            return_type: String::new(),
            span: self.span(prefix.start, self.index.saturating_sub(1).max(prefix.start)),
            supertypes: Vec::new(),
            type_name: String::new(),
        };

        let header_end = self.find_type_header_end();
        if self.at(":") {
            declaration.supertypes = self.parse_supertypes(self.index + 1, header_end);
        }
        self.index = header_end;
        if self.at("{") {
            self.index += 1;
            declaration.children = self.parse_scope(true);
            declaration.span =
                self.span(prefix.start, self.index.saturating_sub(1).max(prefix.start));
        }
        if kind == "type" && keyword == "class" {
            let constructor = Declaration {
                annotations: Vec::new(),
                body: TokenRange::default(),
                children: Vec::new(),
                delegation: TokenRange::default(),
                delegated: false,
                form: "primary_constructor".into(),
                kind: "constructor".into(),
                modifiers: if explicit_constructor {
                    Vec::new()
                } else {
                    vec!["implicit".into()]
                },
                mutable: false,
                name,
                parameters,
                primary: true,
                receiver: String::new(),
                return_type: String::new(),
                span: declaration.span.clone(),
                supertypes: Vec::new(),
                type_name: String::new(),
            };
            declaration.children.insert(0, constructor);
        }
        Some(declaration)
    }

    pub fn parse_function(&mut self, prefix: DeclarationPrefix) -> Option<Declaration> {
        let start = prefix.start;
        self.index += 1;
        self.skip_newlines();
        if self.at("<") {
            self.skip_balanced("<", ">");
            self.skip_newlines();
        }
        let name_start = self.index;
        let Some(open) = self.find_header_token("(") else {
            self.add_diagnostic(start, "function declaration has no parameter list");
            self.skip_statement();
            return None;
        };
        let Some(name_index) = last_identifier(&self.tokens, name_start, open) else {
            self.add_diagnostic(start, "function declaration is missing a name");
            self.index = open;
            self.skip_balanced("(", ")");
            self.skip_statement();
            return None;
        };
        let name = identifier_text(&self.tokens[name_index]);
        let receiver = last_token(&self.tokens, name_start, name_index, ".")
            .map(|dot| self.token_text(name_start, dot))
            .unwrap_or_default();

        self.index = open;
        let parameters = self.parse_parameter_list()?;
        let mut return_type = String::new();
        self.skip_newlines();
        if self.at(":") {
            self.index += 1;
            let type_start = self.index;
            let end = self.find_type_end();
            return_type = self.token_text(type_start, end);
            self.index = end;
        }
        while self.at("where") {
            self.skip_statement();
        }
        let mut body = TokenRange::default();
        if self.at("{") {
            let open = self.index;
            if let Some(close) = self.matching_delimiter(open, "{", "}") {
                body = TokenRange {
                    start: open + 1,
                    end: close,
                };
            }
            self.skip_balanced("{", "}");
        } else if self.at("=") {
            body.start = self.index + 1;
            self.skip_expression();
            body.end = self.index;
        }

        Some(Declaration {
            annotations: prefix.annotations,
            body,
            children: Vec::new(),
            delegation: TokenRange::default(),
            delegated: false,
            form: "function".into(),
            kind: "function".into(),
            modifiers: prefix.modifiers,
            mutable: false,
            name,
            parameters,
            primary: false,
            receiver,
            return_type,
            span: self.span(start, self.index.saturating_sub(1).max(start)),
            supertypes: Vec::new(),
            type_name: String::new(),
        })
    }

    pub fn parse_property(&mut self, prefix: DeclarationPrefix) -> Option<Declaration> {
        let start = prefix.start;
        let mutable = self.at("var");
        self.index += 1;
        self.skip_newlines();
        if self.at("(") {
            self.add_diagnostic(start, "destructuring property declarations are not modeled");
            self.skip_statement();
            return None;
        }
        let header_end = self.find_property_header_end();
        let colon = first_top_level(&self.tokens, self.index, header_end, ":");
        let equals = first_top_level(&self.tokens, self.index, header_end, "=");
        let by = first_top_level(&self.tokens, self.index, header_end, "by");
        let name_end = [colon, equals, by]
            .into_iter()
            .flatten()
            .fold(header_end, usize::min);
        let Some(name_index) = last_identifier(&self.tokens, self.index, name_end) else {
            self.add_diagnostic(start, "property declaration is missing a name");
            self.skip_statement();
            return None;
        };
        let name = identifier_text(&self.tokens[name_index]);
        let receiver = last_token(&self.tokens, self.index, name_index, ".")
            .map(|dot| self.token_text(self.index, dot))
            .unwrap_or_default();
        let type_name = colon
            .map(|colon| {
                let type_end = [equals, by]
                    .into_iter()
                    .flatten()
                    .filter(|candidate| *candidate > colon)
                    .fold(header_end, usize::min);
                self.token_text(colon + 1, type_end)
            })
            .unwrap_or_default();

        self.index = header_end;
        if self.at("=") || self.at("by") {
            self.skip_expression();
        }
        Some(Declaration {
            annotations: prefix.annotations,
            body: TokenRange::default(),
            children: Vec::new(),
            delegation: TokenRange::default(),
            delegated: by.is_some(),
            form: "property".into(),
            kind: "field".into(),
            modifiers: prefix.modifiers,
            mutable,
            name,
            parameters: Vec::new(),
            primary: false,
            receiver,
            return_type: String::new(),
            span: self.span(start, self.index.saturating_sub(1).max(start)),
            supertypes: Vec::new(),
            type_name,
        })
    }

    pub fn parse_constructor(
        &mut self,
        prefix: DeclarationPrefix,
        primary: bool,
    ) -> Option<Declaration> {
        let start = prefix.start;
        self.index += 1;
        self.skip_newlines();
        if !self.at("(") {
            self.add_diagnostic(start, "constructor declaration has no parameter list");
            self.skip_statement();
            return None;
        }
        let parameters = self.parse_parameter_list()?;
        let mut delegation = TokenRange::default();
        while self.current().kind != TokenKind::Eof
            && self.current().kind != TokenKind::Newline
            && !self.at("{")
            && !self.at(";")
        {
            if matches!(self.current().text.as_str(), "this" | "super")
                && self
                    .tokens
                    .get(self.index + 1)
                    .is_some_and(|token| token.text == "(")
                && let Some(close) = self.matching_delimiter(self.index + 1, "(", ")")
            {
                delegation = TokenRange {
                    start: self.index,
                    end: close + 1,
                };
            }
            if self.at("(") {
                self.skip_balanced("(", ")");
            } else {
                self.index += 1;
            }
        }
        let mut body = TokenRange::default();
        if self.at("{") {
            let open = self.index;
            if let Some(close) = self.matching_delimiter(open, "{", "}") {
                body = TokenRange {
                    start: open + 1,
                    end: close,
                };
            }
            self.skip_balanced("{", "}");
        }
        Some(Declaration {
            annotations: prefix.annotations,
            body,
            children: Vec::new(),
            delegation,
            delegated: false,
            form: "secondary_constructor".into(),
            kind: "constructor".into(),
            modifiers: prefix.modifiers,
            mutable: false,
            name: "constructor".into(),
            parameters,
            primary,
            receiver: String::new(),
            return_type: String::new(),
            span: self.span(start, self.index.saturating_sub(1).max(start)),
            supertypes: Vec::new(),
            type_name: String::new(),
        })
    }
}

fn is_visibility(value: &str) -> bool {
    matches!(value, "public" | "private" | "protected" | "internal")
}
