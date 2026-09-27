use std::collections::{BTreeMap, BTreeSet};

use super::super::relationships;
use super::super::shapes::TypeShape;
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn method_targets(&mut self, class_id: &str, method: &str) -> BTreeSet<String> {
        let Some(class) = self.facts.qnames.get(class_id).cloned() else {
            return BTreeSet::new();
        };
        if matches!(self.kind(class_id), Some("interface" | "trait")) {
            let mut result = BTreeSet::new();
            for descendant in self.descendants(&class) {
                if let Some(id) = self.facts.symbols.get(&descendant).cloned()
                    && !matches!(self.kind(&id), Some("interface" | "trait"))
                {
                    result.extend(self.method_targets(&id, method));
                }
            }
            return result;
        }

        for candidate in self.mro_qnames(&class) {
            if let Some(target) = self
                .facts
                .symbols
                .get(&format!("{candidate}.{method}"))
                .cloned()
                && self.kind(&target) == Some("method")
            {
                return BTreeSet::from([target]);
            }
        }
        BTreeSet::new()
    }

    pub(super) fn super_method_targets(
        &mut self,
        class_qname: Option<&str>,
        method: &str,
    ) -> BTreeSet<String> {
        let Some(class_qname) = class_qname else {
            return BTreeSet::new();
        };
        for candidate in self.mro_qnames(class_qname).into_iter().skip(1) {
            if let Some(target) = self
                .facts
                .symbols
                .get(&format!("{candidate}.{method}"))
                .cloned()
                && self.kind(&target) == Some("method")
            {
                return BTreeSet::from([target]);
            }
        }
        BTreeSet::new()
    }

    pub(super) fn annotation_reference_shape(&mut self, id: Option<String>) -> TypeShape {
        let Some(id) = id else {
            return TypeShape::default();
        };
        if !matches!(self.kind(&id), Some("type" | "interface" | "trait")) {
            return TypeShape::default();
        }
        let mut shape = TypeShape::direct(id.clone());
        if let Some(qname) = self.facts.qnames.get(&id).cloned() {
            for descendant in self.descendants(&qname) {
                if let Some(descendant_id) = self.facts.symbols.get(&descendant) {
                    shape.direct.insert(descendant_id.clone());
                }
            }
        }
        shape
    }

    pub(super) fn base_qnames(&mut self, class_qname: &str) -> Vec<String> {
        if let Some(value) = self.base_cache.get(class_qname) {
            return value.clone();
        }
        let value = relationships::base_qnames(self.facts, &mut self.bindings, class_qname);
        self.base_cache
            .insert(class_qname.to_owned(), value.clone());
        value
    }

    fn mro_qnames(&mut self, class_qname: &str) -> Vec<String> {
        if let Some(value) = self.mro_cache.get(class_qname) {
            return value.clone();
        }
        let value = relationships::mro_qnames(self.facts, &mut self.bindings, class_qname);
        self.mro_cache.insert(class_qname.to_owned(), value.clone());
        value
    }

    fn descendants(&mut self, class_qname: &str) -> BTreeSet<String> {
        if let Some(value) = self.descendant_cache.get(class_qname) {
            return value.clone();
        }
        if self.children_by_base.is_none() {
            let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
            let candidates = self.facts.classes.keys().cloned().collect::<Vec<_>>();
            for candidate in candidates {
                for base in self.base_qnames(&candidate) {
                    children.entry(base).or_default().push(candidate.clone());
                }
            }
            self.children_by_base = Some(children);
        }

        let children = self.children_by_base.as_ref().expect("children index");
        let mut result = BTreeSet::new();
        let mut pending = children.get(class_qname).cloned().unwrap_or_default();
        while let Some(candidate) = pending.pop() {
            if !result.insert(candidate.clone()) {
                continue;
            }
            pending.extend(children.get(&candidate).cloned().unwrap_or_default());
        }
        self.descendant_cache
            .insert(class_qname.to_owned(), result.clone());
        result
    }

    pub(super) fn instance_type_ids(&mut self, class_id: &str) -> BTreeSet<String> {
        let Some(qname) = self.facts.qnames.get(class_id).cloned() else {
            return BTreeSet::from([class_id.to_owned()]);
        };
        let mut ids = BTreeSet::from([class_id.to_owned()]);
        for descendant in self.descendants(&qname) {
            if let Some(id) = self.facts.symbols.get(&descendant) {
                ids.insert(id.clone());
            }
        }
        ids
    }
}
