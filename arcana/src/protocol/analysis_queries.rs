use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::repository::{NodeKind, RelationKind};
use crate::synthetic::NodeId;

use super::request::QueryDirection;
use super::response::node_value;
use super::session::{ProtocolSnapshot, RequestFailure};
use super::traversal::{
    bfs_distances, bounded_depth, bounded_result_limit, call_relations, graph_neighbors,
    impact_relations, parse_relations, path_value, related_values, require_entries, require_node,
    shortest_path,
};

impl ProtocolSnapshot {
    pub(crate) fn reachability(
        &self,
        entry_node_ids: &[u32],
        include_possible: bool,
        max_depth: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let entries = require_entries(self, entry_node_ids)?;
        let max_depth = bounded_depth(max_depth);
        let limit = bounded_result_limit(limit);
        let allowed = call_relations(include_possible);
        let distances = bfs_distances(
            self,
            &entries,
            QueryDirection::Outgoing,
            Some(allowed),
            max_depth,
        )?;
        let mut reachable = distances
            .into_iter()
            .enumerate()
            .filter_map(|(node, depth)| depth.map(|depth| (NodeId(node as u32), depth)))
            .collect::<Vec<_>>();
        reachable.sort_unstable_by_key(|(node, depth)| (*depth, *node));
        let total = reachable.len();
        let nodes = reachable
            .into_iter()
            .take(limit)
            .map(|(node, depth)| {
                let entry = self.entry(node)?.ok_or_else(|| {
                    RequestFailure::new(
                        "invalid_snapshot",
                        format!("missing catalogue node {}", node.0),
                    )
                })?;
                Ok(json!({"depth": depth, "node": node_value(&entry)}))
            })
            .collect::<Result<Vec<_>, RequestFailure>>()?;
        Ok(json!({
            "entry_node_ids": entry_node_ids,
            "include_possible": include_possible,
            "max_depth": max_depth,
            "count": total,
            "returned": nodes.len(),
            "truncated": total > nodes.len(),
            "reachable": nodes,
        }))
    }

    pub(crate) fn impact(
        &self,
        node_id: u32,
        relations: Option<&[String]>,
        max_depth: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let source = require_node(self, node_id)?;
        let max_depth = bounded_depth(max_depth);
        let limit = bounded_result_limit(limit);
        let parsed = parse_relations(relations)?;
        let defaults = impact_relations();
        let allowed = parsed.unwrap_or(defaults);
        let distances = bfs_distances(
            self,
            &[source],
            QueryDirection::Incoming,
            Some(allowed),
            max_depth,
        )?;
        let mut impacted = distances
            .into_iter()
            .enumerate()
            .filter_map(|(node, depth)| {
                let node = NodeId(node as u32);
                depth.filter(|_| node != source).map(|depth| (node, depth))
            })
            .collect::<Vec<_>>();
        impacted.sort_unstable_by_key(|(node, depth)| (*depth, *node));
        let total = impacted.len();
        let nodes = impacted
            .into_iter()
            .take(limit)
            .map(|(node, depth)| {
                let entry = self.entry(node)?.ok_or_else(|| {
                    RequestFailure::new(
                        "invalid_snapshot",
                        format!("missing catalogue node {}", node.0),
                    )
                })?;
                Ok(json!({"depth": depth, "node": node_value(&entry)}))
            })
            .collect::<Result<Vec<_>, RequestFailure>>()?;
        Ok(json!({
            "node_id": node_id,
            "max_depth": max_depth,
            "relations": allowed.relation_names(),
            "count": total,
            "returned": nodes.len(),
            "truncated": total > nodes.len(),
            "dependents": nodes,
        }))
    }

    pub(crate) fn dead_symbols(
        &self,
        entry_node_ids: &[u32],
        include_possible: bool,
        kinds: Option<&[String]>,
        max_depth: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let entries = require_entries(self, entry_node_ids)?;
        let max_depth = bounded_depth(max_depth);
        let limit = bounded_result_limit(limit);
        let allowed = call_relations(include_possible);
        let reachable = bfs_distances(
            self,
            &entries,
            QueryDirection::Outgoing,
            Some(allowed),
            max_depth,
        )?;
        let kinds = parse_kinds(kinds)?;
        let mut total = 0;
        let mut nodes = Vec::new();
        let mut candidates = Vec::new();
        for kind in &kinds {
            candidates.extend(self.query.node_ids_by_kind(kind)?);
        }
        candidates.sort_unstable();
        for id in candidates {
            if reachable
                .get(id.0 as usize)
                .is_none_or(|depth| depth.is_none())
            {
                total += 1;
                if nodes.len() < limit {
                    nodes.push(node_value(&self.entry(id)?.expect("indexed node")));
                }
            }
        }
        Ok(json!({
            "entry_node_ids": entry_node_ids,
            "include_possible": include_possible,
            "max_depth": max_depth,
            "kinds": kinds.iter().map(NodeKind::as_str).collect::<Vec<_>>(),
            "count": total,
            "returned": nodes.len(),
            "truncated": total > nodes.len(),
            "dead_symbols": nodes,
        }))
    }

    pub(crate) fn operational_role(
        &self,
        node_id: u32,
        entry_node_ids: Option<&[u32]>,
        include_possible: bool,
        max_depth: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let node = require_node(self, node_id)?;
        let entry = self.entry(node)?.expect("validated node");
        let incoming = graph_neighbors(self, node, QueryDirection::Incoming, None)?;
        let outgoing = graph_neighbors(self, node, QueryDirection::Outgoing, None)?;
        let incoming_counts = relation_counts(&incoming);
        let outgoing_counts = relation_counts(&outgoing);
        let callers = filtered(
            &incoming,
            &[
                RelationKind::Calls,
                RelationKind::PossibleCalls,
                RelationKind::ObservedCalls,
            ],
        );
        let callees = filtered(
            &outgoing,
            &[
                RelationKind::Calls,
                RelationKind::PossibleCalls,
                RelationKind::ObservedCalls,
            ],
        );
        let summary = role_summary(&entry.fact.name, &incoming_counts, &outgoing_counts);

        let mut best_chain = None;
        if let Some(entry_node_ids) = entry_node_ids {
            let entries = require_entries(self, entry_node_ids)?;
            let allowed = call_relations(include_possible);
            let max_depth = bounded_depth(max_depth);
            for start in entries {
                if let Some((nodes, relations)) =
                    shortest_path(self, start, node, Some(allowed), max_depth)?
                {
                    let replace = best_chain
                        .as_ref()
                        .is_none_or(|(depth, _): &(usize, Value)| relations.len() < *depth);
                    if replace {
                        best_chain = Some((relations.len(), path_value(self, &nodes, &relations)?));
                    }
                }
            }
        }
        Ok(json!({
            "node": node_value(&entry),
            "summary": summary,
            "incoming_counts": incoming_counts,
            "outgoing_counts": outgoing_counts,
            "callers": related_values(self, &callers)?,
            "callees": related_values(self, &callees)?,
            "shortest_entry_chain": best_chain.map(|(_, value)| value),
        }))
    }
}

fn parse_kinds(values: Option<&[String]>) -> Result<BTreeSet<NodeKind>, RequestFailure> {
    let Some(values) = values else {
        return Ok(BTreeSet::from([
            NodeKind::Function,
            NodeKind::Method,
            NodeKind::Constructor,
        ]));
    };
    let mut kinds = BTreeSet::new();
    for value in values {
        let kind = NodeKind::parse(value).ok_or_else(|| {
            RequestFailure::new("invalid_node_kind", format!("unknown node kind '{value}'"))
        })?;
        kinds.insert(kind);
    }
    Ok(kinds)
}

fn relation_counts(values: &[(NodeId, RelationKind)]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for (_, relation) in values {
        *counts.entry(relation.as_str()).or_insert(0) += 1;
    }
    counts
}

fn filtered(
    values: &[(NodeId, RelationKind)],
    relations: &[RelationKind],
) -> Vec<(NodeId, RelationKind)> {
    values
        .iter()
        .filter(|(_, relation)| relations.contains(relation))
        .cloned()
        .collect()
}

fn role_summary(
    name: &str,
    incoming: &BTreeMap<&'static str, usize>,
    outgoing: &BTreeMap<&'static str, usize>,
) -> String {
    let callers = incoming.get("calls").copied().unwrap_or(0);
    let possible_callers = incoming.get("possible-calls").copied().unwrap_or(0);
    let observed_callers = incoming.get("observed-calls").copied().unwrap_or(0);
    let callees = outgoing.get("calls").copied().unwrap_or(0);
    let possible_targets = outgoing.get("possible-calls").copied().unwrap_or(0);
    let observed_targets = outgoing.get("observed-calls").copied().unwrap_or(0);
    let references = incoming.get("references").copied().unwrap_or(0);
    format!(
        "{name} has {callers} definite caller(s), {possible_callers} possible caller(s), \
         {observed_callers} runtime-observed caller(s), {callees} definite callee(s), \
         {possible_targets} possible target(s), {observed_targets} runtime-observed target(s), \
         and {references} incoming reference(s)."
    )
}
