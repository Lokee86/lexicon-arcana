use super::model::{ParameterDecl, TokenKind};
use super::parser::Parser;
use super::tokens::{
    annotation_end, first_top_level, identifier_text, last_identifier, skip_kind, split_top_level,
    trim_kind, unique_sorted,
};

const PARAMETER_MODIFIERS: &[&str] = &["crossinline", "noinline", "vararg"];

impl Parser {
    pub fn parse_parameter_list(&mut self) -> Option<Vec<ParameterDecl>> {
        let open = self.index;
        let Some(close) = self.matching_delimiter(open, "(", ")") else {
            self.add_diagnostic(open, "unclosed parameter list");
            self.index = self.find_recovery_boundary(open + 1);
            return None;
        };
        let mut parameters = Vec::new();
        let mut valid = true;
        for (start, end) in split_top_level(&self.tokens, open + 1, close, ",") {
            if let Some(parameter) = self.parse_parameter_segment(start, end) {
                parameters.push(parameter);
            } else if skip_kind(&self.tokens, start, end, TokenKind::Newline) < end {
                valid = false;
            }
        }
        self.index = close + 1;
        valid.then_some(parameters)
    }

    fn parse_parameter_segment(
        &mut self,
        mut start: usize,
        mut end: usize,
    ) -> Option<ParameterDecl> {
        start = skip_kind(&self.tokens, start, end, TokenKind::Newline);
        end = trim_kind(&self.tokens, start, end, TokenKind::Newline);
        if start >= end {
            return None;
        }
        let original_start = start;
        let mut annotations = Vec::new();
        let mut modifiers = Vec::new();
        let mut property = false;
        let mut mutable = false;

        while start < end {
            if self.tokens[start].text == "@" {
                let annotation_end = annotation_end(&self.tokens, start, end);
                annotations.push(super::tokens::compact_range(
                    &self.tokens,
                    start + 1,
                    annotation_end,
                ));
                start = annotation_end;
                continue;
            }
            if PARAMETER_MODIFIERS.contains(&self.tokens[start].text.as_str()) {
                modifiers.push(self.tokens[start].text.clone());
                start += 1;
                continue;
            }
            if matches!(self.tokens[start].text.as_str(), "val" | "var") {
                property = true;
                mutable = self.tokens[start].text == "var";
                start += 1;
                continue;
            }
            break;
        }

        let Some(colon) = first_top_level(&self.tokens, start, end, ":") else {
            self.add_diagnostic(original_start, "parameter declaration has no type");
            return None;
        };
        let Some(name_index) = last_identifier(&self.tokens, start, colon) else {
            self.add_diagnostic(original_start, "parameter declaration is missing a name");
            return None;
        };
        let equals = first_top_level(&self.tokens, colon + 1, end, "=");
        let type_end = equals.unwrap_or(end);
        Some(ParameterDecl {
            annotations: unique_sorted(annotations),
            has_default: equals.is_some(),
            modifiers: unique_sorted(modifiers),
            mutable,
            name: identifier_text(&self.tokens[name_index]),
            property,
            span: self.span(original_start, end - 1),
            type_name: self.token_text(colon + 1, type_end),
        })
    }
}
