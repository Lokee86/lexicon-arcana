use std::collections::BTreeSet;

use super::{
    LookupDirection, LookupEdge, LookupError, LookupNode, LookupReference, LookupUnresolved,
    SnapshotLookup,
};

impl SnapshotLookup {
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    pub fn find(&self, query: &str, limit: usize) -> Vec<LookupNode> {
        let query = query.trim();
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }
        let lower = query.to_lowercase();
        let mut matches = self
            .nodes
            .values()
            .filter_map(|node| match_rank(node, query, &lower).map(|rank| (rank, node)))
            .collect::<Vec<_>>();
        matches.sort_by(|(left_rank, left), (right_rank, right)| {
            left_rank
                .cmp(right_rank)
                .then_with(|| left.node.qualified_name.cmp(&right.node.qualified_name))
                .then_with(|| left.node.path.cmp(&right.node.path))
                .then_with(|| left.node.id.cmp(&right.node.id))
        });
        matches
            .into_iter()
            .take(limit)
            .map(|(_, node)| node.clone())
            .collect()
    }

    pub fn resolve(&self, selector: &str) -> Result<LookupNode, LookupError> {
        if let Some(node) = self.nodes.get(selector) {
            return Ok(node.clone());
        }
        let candidates = self
            .nodes
            .values()
            .filter(|node| {
                node.node.qualified_name == selector
                    || node.node.name == selector
                    || node.node.path == selector
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [node] => Ok((*node).clone()),
            [] => Err(LookupError::NotFound(selector.to_owned())),
            _ => Err(LookupError::Ambiguous {
                selector: selector.to_owned(),
                candidates: candidates
                    .iter()
                    .map(|node| node.node.id.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            }),
        }
    }

    pub fn refs(&self, selector: &str, limit: usize) -> Result<Vec<LookupReference>, LookupError> {
        if !self.relationships_loaded {
            return Err(LookupError::RelationshipsNotLoaded);
        }
        let node = self.resolve(selector)?;
        Ok(self.references_for(&node.node.id, limit, |_| true))
    }

    pub fn calls(&self, selector: &str, limit: usize) -> Result<Vec<LookupReference>, LookupError> {
        if !self.relationships_loaded {
            return Err(LookupError::RelationshipsNotLoaded);
        }
        let node = self.resolve(selector)?;
        Ok(self.references_for(&node.node.id, limit, is_call_relation))
    }

    fn references_for(
        &self,
        id: &str,
        limit: usize,
        include: impl Fn(&str) -> bool,
    ) -> Vec<LookupReference> {
        if limit == 0 {
            return Vec::new();
        }
        let mut values = Vec::new();
        for stored in &self.edges {
            if !include(&stored.record.relation) {
                continue;
            }
            let direction = if stored.record.source == id {
                Some(LookupDirection::Outgoing)
            } else if stored.record.target == id {
                Some(LookupDirection::Incoming)
            } else {
                None
            };
            if let Some(direction) = direction {
                values.push(LookupReference::Edge(Box::new(LookupEdge {
                    language: stored.language.clone(),
                    direction,
                    source: self.nodes.get(&stored.record.source).cloned(),
                    target: self.nodes.get(&stored.record.target).cloned(),
                    edge: stored.record.clone(),
                })));
            }
        }
        for stored in &self.unresolved {
            if stored.record.source == id && include(&stored.record.relation) {
                values.push(LookupReference::Unresolved(Box::new(LookupUnresolved {
                    language: stored.language.clone(),
                    record: stored.record.clone(),
                })));
            }
        }
        values.sort_by_key(reference_key);
        values.truncate(limit);
        values
    }
}

fn match_rank(node: &LookupNode, query: &str, lower: &str) -> Option<u8> {
    let fields = [
        node.node.qualified_name.as_str(),
        node.node.name.as_str(),
        node.node.path.as_str(),
        node.node.kind.as_str(),
    ];
    if fields.contains(&query) {
        return Some(0);
    }
    let lowered = fields.map(str::to_lowercase);
    if lowered.iter().any(|value| value == lower) {
        return Some(1);
    }
    if lowered.iter().any(|value| value.starts_with(lower)) {
        return Some(2);
    }
    lowered
        .iter()
        .any(|value| value.contains(lower))
        .then_some(3)
}

fn is_call_relation(relation: &str) -> bool {
    matches!(relation, "calls" | "possible-calls" | "calls-endpoint")
}

fn reference_key(reference: &LookupReference) -> (u8, String, String, String) {
    match reference {
        LookupReference::Edge(value) => {
            let other = match value.direction {
                LookupDirection::Outgoing => value.edge.target.clone(),
                LookupDirection::Incoming => value.edge.source.clone(),
            };
            (
                match value.direction {
                    LookupDirection::Outgoing => 0,
                    LookupDirection::Incoming => 1,
                },
                value.edge.relation.clone(),
                other,
                value.language.clone(),
            )
        }
        LookupReference::Unresolved(value) => (
            2,
            value.record.relation.clone(),
            value.record.expression.clone(),
            value.language.clone(),
        ),
    }
}
