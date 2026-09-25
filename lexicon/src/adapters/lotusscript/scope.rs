use std::collections::{BTreeSet, VecDeque};

use serde_json::json;

use super::facts::Facts;
use super::model::{AnalysisState, Declaration, VariableSymbol};

impl AnalysisState {
    pub fn resolve_uses(&mut self, facts: &mut Facts) {
        self.uses.sort_by(|left, right| {
            left.owner_path
                .cmp(&right.owner_path)
                .then_with(|| left.span.start_line.cmp(&right.span.start_line))
        });
        for evidence in self.uses.clone() {
            if evidence.dynamic {
                facts.add_unresolved(
                    &evidence.import_id,
                    "imports",
                    &evidence.expression,
                    "dynamic-target",
                    Some(&evidence.owner_path),
                    Some(evidence.span),
                    None,
                );
                continue;
            }
            if evidence.keyword.eq_ignore_ascii_case("UseLSX") {
                facts.add_unresolved(
                    &evidence.import_id,
                    "imports",
                    &evidence.target,
                    "external-target",
                    Some(&evidence.owner_path),
                    Some(evidence.span),
                    Some(json!({"extension": true})),
                );
                continue;
            }

            let key = evidence.target.to_ascii_lowercase();
            let mut candidates = self.modules_by_name.get(&key).cloned().unwrap_or_default();
            candidates.sort();
            match candidates.as_slice() {
                [] => facts.add_unresolved(
                    &evidence.import_id,
                    "imports",
                    &evidence.target,
                    "external-target",
                    Some(&evidence.owner_path),
                    Some(evidence.span),
                    None,
                ),
                [target] => {
                    facts.add_edge(
                        &evidence.import_id,
                        target,
                        "imports",
                        Some(&evidence.owner_path),
                        Some(evidence.span.clone()),
                        None,
                    );
                    if let Some(paths) = self.module_paths_by_name.get(&key)
                        && paths.len() == 1
                    {
                        push_unique(
                            self.imports_by_path
                                .entry(evidence.owner_path.clone())
                                .or_default(),
                            paths[0].clone(),
                        );
                    }
                }
                _ => facts.add_unresolved(
                    &evidence.import_id,
                    "imports",
                    &evidence.target,
                    "ambiguous-target",
                    Some(&evidence.owner_path),
                    Some(evidence.span),
                    Some(json!({"candidate_count": candidates.len()})),
                ),
            }
        }
    }

    pub fn resolve_extends(&mut self, facts: &mut Facts) {
        self.extends.sort_by(|left, right| {
            left.owner_path
                .cmp(&right.owner_path)
                .then_with(|| left.span.start_line.cmp(&right.span.start_line))
        });
        for evidence in self.extends.clone() {
            let key = evidence.base.to_ascii_lowercase();
            let candidates = self.visible_declarations(
                &evidence.owner_path,
                self.classes_by_name.get(&key).cloned().unwrap_or_default(),
            );
            match candidates.as_slice() {
                [] => facts.add_unresolved(
                    &evidence.class_id,
                    "extends",
                    &evidence.base,
                    "external-target",
                    Some(&evidence.owner_path),
                    Some(evidence.span),
                    None,
                ),
                [target] => {
                    facts.add_edge(
                        &evidence.class_id,
                        &target.id,
                        "extends",
                        Some(&evidence.owner_path),
                        Some(evidence.span),
                        None,
                    );
                    push_unique(
                        self.bases_by_class.entry(evidence.class_id).or_default(),
                        target.id.clone(),
                    );
                }
                _ => facts.add_unresolved(
                    &evidence.class_id,
                    "extends",
                    &evidence.base,
                    "ambiguous-target",
                    Some(&evidence.owner_path),
                    Some(evidence.span),
                    Some(json!({"candidate_count": candidates.len()})),
                ),
            }
        }
    }

    pub fn visible_paths(&self, owner_path: &str) -> BTreeSet<String> {
        let mut visible = BTreeSet::from([owner_path.to_owned()]);
        let mut queue = VecDeque::from([owner_path.to_owned()]);
        while let Some(current) = queue.pop_front() {
            for imported in self.imports_by_path.get(&current).into_iter().flatten() {
                if visible.insert(imported.clone()) {
                    queue.push_back(imported.clone());
                }
            }
        }
        visible
    }

    pub fn visible_declarations(
        &self,
        owner_path: &str,
        declarations: Vec<Declaration>,
    ) -> Vec<Declaration> {
        let visible = self.visible_paths(owner_path);
        let mut result = declarations
            .into_iter()
            .filter(|declaration| {
                visible.contains(&declaration.owner_path)
                    && (declaration.owner_path == owner_path || declaration.public)
            })
            .collect::<Vec<_>>();
        result.sort_by(|left, right| left.id.cmp(&right.id));
        result
    }

    pub fn resolve_variable_symbol(
        &self,
        owner_id: &str,
        class_id: Option<&str>,
        owner_path: &str,
        name: &str,
    ) -> Option<VariableSymbol> {
        let name = name.to_ascii_lowercase();
        if let Some(symbol) = self
            .variable_symbols
            .get(owner_id)
            .and_then(|values| values.get(&name))
        {
            return Some(symbol.clone());
        }
        if let Some(class_id) = class_id {
            let candidates = self.field_candidates(class_id, &name, &mut BTreeSet::new());
            if candidates.len() == 1 {
                return candidates.into_iter().next();
            }
        }
        if let Some(symbol) = self
            .module_symbols
            .get(owner_path)
            .and_then(|values| values.get(&name))
        {
            return Some(symbol.clone());
        }

        let mut candidates = Vec::new();
        for path in self.visible_paths(owner_path) {
            if path == owner_path {
                continue;
            }
            if let Some(symbol) = self
                .module_symbols
                .get(&path)
                .and_then(|values| values.get(&name))
                && symbol.public
            {
                candidates.push(symbol.clone());
            }
        }
        (candidates.len() == 1).then(|| candidates.remove(0))
    }

    pub fn field_candidates(
        &self,
        class_id: &str,
        name: &str,
        visited: &mut BTreeSet<String>,
    ) -> Vec<VariableSymbol> {
        if class_id.is_empty() || !visited.insert(class_id.into()) {
            return Vec::new();
        }
        if let Some(symbol) = self
            .field_symbols
            .get(class_id)
            .and_then(|values| values.get(&name.to_ascii_lowercase()))
        {
            return vec![symbol.clone()];
        }
        self.bases_by_class
            .get(class_id)
            .into_iter()
            .flatten()
            .flat_map(|base| self.field_candidates(base, name, visited))
            .collect()
    }

    pub fn class_by_id(&self, id: &str) -> Option<Declaration> {
        self.classes_by_name
            .values()
            .flatten()
            .find(|declaration| declaration.id == id)
            .cloned()
    }

    pub fn class_is_current_or_base(
        &self,
        current: &str,
        target: &str,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if current.is_empty() || target.is_empty() {
            return false;
        }
        if current == target {
            return true;
        }
        if !visited.insert(current.into()) {
            return false;
        }
        self.bases_by_class
            .get(current)
            .into_iter()
            .flatten()
            .any(|base| self.class_is_current_or_base(base, target, visited))
    }
}

pub fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}
