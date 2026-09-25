use std::collections::BTreeSet;

use super::model::{Token, TokenKind, TokenRange};
use super::runtime::RuntimeCallable;
use super::runtime_tokens::{
    callee_start, nested_ranges, next_token, previous_token, token_ignored, token_span,
};
use super::runtime_values::shadowed_names;
use super::state::AnalysisState;
use super::tokens::{compact_range, identifier_text, last_token};

impl AnalysisState {
    pub fn emit_callable_dataflow(&mut self, callable: &RuntimeCallable) {
        let shadowed = shadowed_names(callable);
        for bounds in [
            callable.declaration.delegation.clone(),
            callable.declaration.body.clone(),
        ] {
            self.emit_callable_range_dataflow(callable, &bounds, &shadowed);
        }
    }

    fn emit_callable_range_dataflow(
        &mut self,
        callable: &RuntimeCallable,
        bounds: &TokenRange,
        shadowed: &BTreeSet<String>,
    ) {
        if bounds.end <= bounds.start {
            return;
        }
        let tokens = &callable.file.tokens;
        let ignored = nested_ranges(tokens, bounds);
        for index in bounds.start..bounds.end.min(tokens.len()) {
            if token_ignored(index, &ignored)
                || tokens[index].kind != TokenKind::Identifier
                || reference_declaration(tokens, index, bounds.start)
            {
                continue;
            }
            let (start, unsupported) = callee_start(tokens, index, bounds.start);
            if unsupported {
                continue;
            }
            let qualifier = last_token(tokens, start, index, ".")
                .map(|dot| compact_range(tokens, start, dot))
                .unwrap_or_default();
            let name = identifier_text(&tokens[index]);
            if matches!(name.as_str(), "this" | "super") || named_argument(tokens, index, bounds) {
                continue;
            }
            let target = self.resolve_runtime_value(callable, &qualifier, &name, shadowed);
            if target.is_empty() {
                continue;
            }
            let span = token_span(&callable.file.path, &tokens[start], &tokens[index]);
            let (write, compound) = write_kind(tokens, start, index, bounds);
            if !write || compound {
                self.facts.add_edge(
                    &callable.id,
                    &target,
                    "reads",
                    Some(&callable.file.path),
                    Some(span.clone()),
                    None,
                );
            }
            if write {
                self.facts.add_edge(
                    &callable.id,
                    &target,
                    "writes",
                    Some(&callable.file.path),
                    Some(span),
                    None,
                );
            }
        }
    }
}

fn reference_declaration(tokens: &[Token], index: usize, lower: usize) -> bool {
    previous_token(tokens, index as isize - 1, lower).is_some_and(|previous| {
        matches!(
            tokens[previous].text.as_str(),
            "class" | "fun" | "object" | "val" | "var"
        )
    })
}

fn named_argument(tokens: &[Token], index: usize, bounds: &TokenRange) -> bool {
    let next = next_token(tokens, index + 1, bounds.end);
    if next >= bounds.end || tokens[next].text != "=" {
        return false;
    }
    let after = next_token(tokens, next + 1, bounds.end);
    if after < bounds.end && tokens[after].text == "=" {
        return false;
    }
    previous_token(tokens, index as isize - 1, bounds.start)
        .is_some_and(|previous| matches!(tokens[previous].text.as_str(), "(" | ","))
}

fn write_kind(tokens: &[Token], start: usize, name: usize, bounds: &TokenRange) -> (bool, bool) {
    let next = next_token(tokens, name + 1, bounds.end);
    if next < bounds.end {
        let after = next_token(tokens, next + 1, bounds.end);
        if tokens[next].text == "=" && (after >= bounds.end || tokens[after].text != "=") {
            return (true, false);
        }
        if matches!(tokens[next].text.as_str(), "+" | "-" | "*" | "/" | "%")
            && after < bounds.end
            && tokens[after].text == "="
        {
            return (true, true);
        }
        if matches!(tokens[next].text.as_str(), "+" | "-")
            && after < bounds.end
            && tokens[after].text == tokens[next].text
        {
            return (true, true);
        }
    }
    let Some(previous) = previous_token(tokens, start as isize - 1, bounds.start) else {
        return (false, false);
    };
    if !matches!(tokens[previous].text.as_str(), "+" | "-") {
        return (false, false);
    }
    if previous_token(tokens, previous as isize - 1, bounds.start)
        .is_some_and(|before| tokens[before].text == tokens[previous].text)
    {
        return (true, true);
    }
    (false, false)
}
