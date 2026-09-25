use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::SourceSpan;

use super::facts::Facts;
use super::model::{AccessEvidence, AnalysisState, Declaration, VariableSymbol};
use super::syntax::mask_literals;

static IDENTIFIER_CHAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*").unwrap());

pub fn collect_access(
    state: &mut AnalysisState,
    span: &SourceSpan,
    text: &str,
    callable: &Declaration,
) {
    state.accesses.push(AccessEvidence {
        class_id: callable.class_id.clone(),
        owner_id: callable.id.clone(),
        owner_path: callable.owner_path.clone(),
        span: span.clone(),
        text: text.into(),
    });
}

impl AnalysisState {
    pub fn resolve_accesses(&mut self, facts: &mut Facts) {
        self.accesses.sort_by(|left, right| {
            left.owner_path
                .cmp(&right.owner_path)
                .then_with(|| left.span.start_line.cmp(&right.span.start_line))
                .then_with(|| left.text.cmp(&right.text))
        });
        for evidence in self.accesses.clone() {
            self.resolve_access(facts, &evidence);
        }
    }

    fn resolve_access(&self, facts: &mut Facts, evidence: &AccessEvidence) {
        let masked = mask_literals(&evidence.text);
        if let Some(assignment) = parse_assignment(&masked) {
            self.add_write(facts, evidence, &assignment.target);
            self.add_reads(facts, evidence, &assignment.left);
            self.add_reads(facts, evidence, &assignment.right);
        } else {
            self.add_reads(facts, evidence, &masked);
        }
    }

    fn add_write(&self, facts: &mut Facts, evidence: &AccessEvidence, target: &str) {
        let parts = split_chain(target);
        if parts.is_empty() {
            return;
        }
        if parts.len() == 1 {
            if let Some(symbol) = self.resolve_variable_symbol(
                &evidence.owner_id,
                evidence.class_id.as_deref(),
                &evidence.owner_path,
                &parts[0],
            ) {
                self.edge(facts, evidence, &symbol, "writes");
            }
            return;
        }

        if parts[0] != "me"
            && let Some(symbol) = self.resolve_variable_symbol(
                &evidence.owner_id,
                evidence.class_id.as_deref(),
                &evidence.owner_path,
                &parts[0],
            )
        {
            self.edge(facts, evidence, &symbol, "reads");
        }
        if let Some(symbol) = self.resolve_member_symbol(evidence, &parts) {
            self.edge(facts, evidence, &symbol, "writes");
        }
    }

    fn add_reads(&self, facts: &mut Facts, evidence: &AccessEvidence, text: &str) {
        for matched in IDENTIFIER_CHAIN.find_iter(text) {
            let parts = split_chain(matched.as_str());
            if parts.is_empty() {
                continue;
            }
            if parts.len() == 1 {
                if let Some(symbol) = self.resolve_variable_symbol(
                    &evidence.owner_id,
                    evidence.class_id.as_deref(),
                    &evidence.owner_path,
                    &parts[0],
                ) {
                    self.edge(facts, evidence, &symbol, "reads");
                }
                continue;
            }

            if parts[0] != "me"
                && let Some(symbol) = self.resolve_variable_symbol(
                    &evidence.owner_id,
                    evidence.class_id.as_deref(),
                    &evidence.owner_path,
                    &parts[0],
                )
            {
                self.edge(facts, evidence, &symbol, "reads");
            }
            if let Some(symbol) = self.resolve_member_symbol(evidence, &parts) {
                self.edge(facts, evidence, &symbol, "reads");
            }
        }
    }

    fn resolve_member_symbol(
        &self,
        evidence: &AccessEvidence,
        parts: &[String],
    ) -> Option<VariableSymbol> {
        if parts.len() < 2 {
            return None;
        }

        let mut classes = if parts[0] == "me" {
            evidence
                .class_id
                .as_deref()
                .and_then(|id| self.class_by_id(id))
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            let receiver = self.resolve_variable_symbol(
                &evidence.owner_id,
                evidence.class_id.as_deref(),
                &evidence.owner_path,
                &parts[0],
            )?;
            if receiver.data_type.is_empty() {
                return None;
            }
            self.visible_declarations(
                &evidence.owner_path,
                self.classes_by_name
                    .get(&receiver.data_type)
                    .cloned()
                    .unwrap_or_default(),
            )
        };
        if classes.len() != 1 {
            return None;
        }

        for (index, member) in parts.iter().enumerate().skip(1) {
            let class = &classes[0];
            let mut candidates = self.field_candidates(&class.id, member, &mut BTreeSet::new());
            let allow_private = parts[0] == "me"
                || evidence.class_id.as_deref().is_some_and(|current| {
                    self.class_is_current_or_base(current, &class.id, &mut BTreeSet::new())
                });
            if !allow_private {
                candidates.retain(|candidate| candidate.public);
            }
            if candidates.len() != 1 {
                return None;
            }
            if index == parts.len() - 1 {
                return candidates.into_iter().next();
            }
            if candidates[0].data_type.is_empty() {
                return None;
            }
            classes = self.visible_declarations(
                &evidence.owner_path,
                self.classes_by_name
                    .get(&candidates[0].data_type)
                    .cloned()
                    .unwrap_or_default(),
            );
            if classes.len() != 1 {
                return None;
            }
        }
        None
    }

    fn edge(
        &self,
        facts: &mut Facts,
        evidence: &AccessEvidence,
        symbol: &VariableSymbol,
        relation: &str,
    ) {
        facts.add_edge(
            &evidence.owner_id,
            &symbol.id,
            relation,
            Some(&evidence.owner_path),
            Some(evidence.span.clone()),
            None,
        );
    }
}

struct Assignment {
    target: String,
    left: String,
    right: String,
}

fn parse_assignment(value: &str) -> Option<Assignment> {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("redim ") {
        let mut rest = trimmed[5..].trim();
        if rest.to_ascii_lowercase().starts_with("preserve ") {
            rest = rest[9..].trim();
        }
        let matched = IDENTIFIER_CHAIN.find(rest)?;
        if matched.start() != 0 {
            return None;
        }
        return Some(Assignment {
            target: matched.as_str().into(),
            left: rest[matched.end()..].into(),
            right: String::new(),
        });
    }

    let mut working = trimmed;
    for prefix in ["set ", "let ", "for "] {
        if working.to_ascii_lowercase().starts_with(prefix) {
            working = working[prefix.len()..].trim();
            break;
        }
    }
    let lower = working.to_ascii_lowercase();
    if [
        "if ", "elseif ", "while ", "until ", "do ", "loop ", "case ", "select ", "on ", "call ",
        "print ", "error ", "return ", "exit ", "with ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
    {
        return None;
    }

    let equals = assignment_equals(working)?;
    let left = working[..equals].trim();
    let right = working[equals + 1..].trim();
    let matched = IDENTIFIER_CHAIN.find(left)?;
    if matched.start() != 0 {
        return None;
    }
    let remainder = left[matched.end()..].trim();
    if !remainder.is_empty() && !remainder.starts_with('(') {
        return None;
    }
    Some(Assignment {
        target: matched.as_str().into(),
        left: remainder.into(),
        right: right.into(),
    })
}

fn assignment_equals(value: &str) -> Option<usize> {
    let mut depth = 0_i32;
    for (index, current) in value.bytes().enumerate() {
        match current {
            b'(' => depth += 1,
            b')' if depth > 0 => depth -= 1,
            b'=' if depth == 0
                && !matches!(
                    value.as_bytes().get(index.wrapping_sub(1)),
                    Some(b'<' | b'>')
                ) =>
            {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn split_chain(value: &str) -> Vec<String> {
    value
        .split('.')
        .map(|part| part.trim().to_ascii_lowercase())
        .filter(|part| !part.is_empty())
        .collect()
}
