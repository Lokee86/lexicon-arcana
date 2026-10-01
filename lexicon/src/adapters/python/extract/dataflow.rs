use rustpython_parser::ast;

use crate::SourceSpan;

use super::super::model::{LocalAssignmentInfo, LoopBindingInfo};
use super::super::source::{end_offset, offset, span};
use super::Visitor;

impl Visitor<'_> {
    pub(super) fn declare_data(
        &mut self,
        name: &str,
        kind: &str,
        record_span: Option<SourceSpan>,
    ) -> String {
        let owner = self.owner().to_owned();
        let key = (owner.clone(), name.to_owned());
        if let Some(existing) = self.facts.data_symbols.get(&key) {
            return existing.clone();
        }
        let owner_qname = self
            .facts
            .qnames
            .get(&owner)
            .cloned()
            .unwrap_or_else(|| self.file.module.clone());
        let qname = format!("{owner_qname}.{name}");
        let id = self.facts.add_node(
            kind,
            name,
            &self.file.relative,
            &qname,
            Some(&format!("{owner}:{name}")),
            record_span.clone(),
            None,
            None,
        );
        self.facts.data_symbols.insert(key, id.clone());
        self.facts
            .add_edge(&owner, &id, "defines", record_span, None);
        id
    }

    pub(super) fn record_assignment(
        &mut self,
        target: &ast::Expr,
        value: Option<&ast::Expr>,
        annotation: Option<&ast::Expr>,
        node: &impl ast::Ranged,
    ) {
        let Some(name) = target_name(target) else {
            self.visit_target(target, false);
            return;
        };
        let kind = if self.class_qname().is_some()
            && self.lexical.last().is_some_and(|(_, class)| *class)
        {
            "field"
        } else {
            "variable"
        };
        self.declare_data(&name, kind, self.node_span(target));
        self.facts.local_assignments.push(LocalAssignmentInfo {
            module_name: self.file.module.clone(),
            scope_id: self.owner().to_owned(),
            class_qname: self.class_qname().map(str::to_owned),
            name,
            value: value.cloned(),
            annotation: annotation.cloned(),
            start: offset(node),
            end: end_offset(node),
            branch_dependent: self.branch_depth > 0,
            direct_class_field: self.direct_class_statement && matches!(target, ast::Expr::Name(_)),
        });
        self.visit_target(target, false);
    }

    pub(super) fn record_loop_targets(
        &mut self,
        target: &ast::Expr,
        iterable: &ast::Expr,
        node: &impl ast::Ranged,
    ) {
        for (name, element_index) in target_bindings(target) {
            self.facts.loop_bindings.push(LoopBindingInfo {
                module_name: self.file.module.clone(),
                scope_id: self.owner().to_owned(),
                class_qname: self.class_qname().map(str::to_owned),
                name: name.clone(),
                start: offset(node),
                iterable: iterable.clone(),
                branch_dependent: self.branch_depth > 0,
                element_index,
            });
            self.declare_data(&name, "variable", self.node_span(target));
            self.emit_dataflow(target, "writes", &name);
        }
    }

    pub(super) fn emit_dataflow(&mut self, node: &ast::Expr, relation: &str, name: &str) {
        let Some(symbol) = self.resolve_data(name) else {
            return;
        };
        let owner = self.owner().to_owned();
        self.facts
            .add_edge(&owner, &symbol, relation, span(node, self.file), None);
    }

    pub(super) fn resolve_data(&self, name: &str) -> Option<String> {
        let mut owner = Some(self.owner().to_owned());
        while let Some(current) = owner {
            if let Some(symbol) = self
                .facts
                .data_symbols
                .get(&(current.clone(), name.to_owned()))
            {
                return Some(symbol.clone());
            }
            owner = self.facts.scope_parents.get(&current).cloned();
        }
        None
    }
}

pub(super) fn target_name(target: &ast::Expr) -> Option<String> {
    match target {
        ast::Expr::Name(value) => Some(value.id.to_string()),
        ast::Expr::Attribute(value) => {
            let parent = target_name(&value.value)?;
            Some(format!("{parent}.{}", value.attr))
        }
        _ => None,
    }
}

pub(super) fn target_bindings(target: &ast::Expr) -> Vec<(String, Option<usize>)> {
    match target {
        ast::Expr::Name(value) => vec![(value.id.to_string(), None)],
        ast::Expr::Tuple(value) => value
            .elts
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                target_bindings(item)
                    .into_iter()
                    .map(move |(name, _)| (name, Some(index)))
            })
            .collect(),
        ast::Expr::List(value) => value
            .elts
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                target_bindings(item)
                    .into_iter()
                    .map(move |(name, _)| (name, Some(index)))
            })
            .collect(),
        _ => Vec::new(),
    }
}
