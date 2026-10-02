use serde_json::{Value, json};

use crate::repository::{
    NodeKind, RelationKind, UnresolvedReason, edge_kind_to_relation, normalize_repository_path,
};
use crate::synthetic::NodeId;

use super::request::QueryDirection;
use super::response::{node_value, relationship_value, unresolved_value};
use super::session::{ProtocolSnapshot, RequestFailure};
use super::traversal::parse_relations;

const DEFAULT_LIMIT: usize = 1_000;
const MAX_LIMIT: usize = 10_000;

impl ProtocolSnapshot {
    pub(crate) fn search_nodes(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Ok(json!({
                "count": 0,
                "returned": 0,
                "truncated": false,
                "matches": [],
            }));
        }

        let normalized_query = query.replace('\\', "/");
        let limit = bounded_limit(limit);
        let (count, matches) = self.query.search_matches(&normalized_query, limit)?;
        let matches = matches
            .into_iter()
            .take(limit)
            .map(|(rank, matched_fields, entry)| {
                json!({
                    "rank": rank,
                    "matched_fields": matched_fields,
                    "node": node_value(&entry),
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "count": count,
            "returned": matches.len(),
            "truncated": count > matches.len(),
            "matches": matches,
        }))
    }

    pub(crate) fn resolve_symbol(
        &self,
        name: &str,
        kind: Option<&str>,
        path: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let kind = parse_kind(kind)?;
        let path = normalize_optional_path(path)?;
        let mut matches = self.query.node_ids_by_name(name)?;
        if let Some(kind) = kind {
            matches = intersect_sorted_ids(&matches, &self.query.node_ids_by_kind(&kind)?);
        }
        if let Some(path) = path {
            matches = intersect_sorted_ids(
                &matches,
                &self
                    .query
                    .node_ids_by_path(&path)
                    .map_err(|error| RequestFailure::new("invalid_path", error.to_string()))?,
            );
        }
        node_list(self, matches, limit)
    }

    pub(crate) fn resolve_file(
        &self,
        path: &str,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let matches = self
            .query
            .node_ids_by_path(path)
            .map_err(|error| RequestFailure::new("invalid_path", error.to_string()))?
            .to_vec();
        let matches =
            intersect_sorted_ids(&matches, &self.query.node_ids_by_kind(&NodeKind::File)?);
        node_list(self, matches, limit)
    }

    pub(crate) fn list_nodes(
        &self,
        kind: Option<&str>,
        path_prefix: Option<&str>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let kind = parse_kind(kind)?;
        let path_prefix = normalize_optional_path(path_prefix)?;
        let matches = match (kind, path_prefix) {
            (None, None) => (0..self.query.len())
                .map(|node_id| NodeId(node_id as u32))
                .collect(),
            (Some(kind), None) => self.query.node_ids_by_kind(&kind)?,
            (None, Some(prefix)) => self
                .query
                .node_ids_by_path_prefix(&prefix)
                .map_err(|error| RequestFailure::new("invalid_path", error.to_string()))?,
            (Some(kind), Some(prefix)) => {
                let path_ids = self
                    .query
                    .node_ids_by_path_prefix(&prefix)
                    .map_err(|error| RequestFailure::new("invalid_path", error.to_string()))?;
                intersect_sorted_ids(&path_ids, &self.query.node_ids_by_kind(&kind)?)
            }
        };
        node_list_page(self, matches, offset, limit)
    }

    pub(crate) fn neighbors(
        &self,
        node_id: u32,
        direction: QueryDirection,
        relation: Option<&str>,
        relations: Option<&[String]>,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let node_id = NodeId(node_id);
        let source = self.entry(node_id)?.ok_or_else(|| {
            RequestFailure::new("unknown_node", format!("node {node_id:?} does not exist"))
        })?;
        let wanted = parse_relation(relation)?;
        let wanted_many = parse_relations(relations)?;
        let neighbors = match direction {
            QueryDirection::Outgoing => self.query.graph().forward_neighbors_iter(node_id),
            QueryDirection::Incoming => self.query.graph().reverse_neighbors_iter(node_id),
        }
        .map_err(|error| RequestFailure::new("query_failed", error.to_string()))?;

        let limit = bounded_limit(limit);
        let scan_limit = limit.saturating_mul(16).clamp(1_024, 16_384);
        let mut scanned = 0usize;
        let mut matched = 0usize;
        let mut truncated = false;
        let mut relationships = Vec::new();
        for neighbor in neighbors {
            if scanned >= scan_limit {
                truncated = true;
                break;
            }
            scanned += 1;
            let relation = edge_kind_to_relation(neighbor.kind).ok_or_else(|| {
                RequestFailure::new(
                    "corrupt_graph",
                    format!("unknown edge kind {}", neighbor.kind.0),
                )
            })?;
            if wanted_many.is_some_and(|wanted| !wanted.contains(&relation)) {
                continue;
            }
            if wanted_many.is_none() && wanted.as_ref().is_some_and(|wanted| wanted != &relation) {
                continue;
            }
            matched += 1;
            if relationships.len() >= limit {
                truncated = true;
                break;
            }
            let entry = self.entry(neighbor.node)?.ok_or_else(|| {
                RequestFailure::new(
                    "invalid_snapshot",
                    format!("catalogue is missing graph node {}", neighbor.node.0),
                )
            })?;
            relationships.push(relationship_value(&relation, &entry));
        }
        Ok(json!({
            "node": node_value(&source),
            "direction": match direction {
                QueryDirection::Incoming => "incoming",
                QueryDirection::Outgoing => "outgoing",
            },
            "count": matched,
            "returned": relationships.len(),
            "scanned": scanned,
            "truncated": truncated,
            "relationships": relationships,
        }))
    }

    pub(crate) fn query_unresolved(
        &self,
        node_id: Option<u32>,
        path: Option<&str>,
        reason: Option<&str>,
        relation: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let source_key = match node_id {
            Some(id) => Some(
                self.entry(NodeId(id))?
                    .ok_or_else(|| {
                        RequestFailure::new("unknown_node", format!("node {id} does not exist"))
                    })?
                    .fact
                    .key,
            ),
            None => None,
        };
        let path = normalize_optional_path(path)?;
        let reason = parse_reason(reason)?;
        let relation = parse_relation(relation)?;
        let limit = bounded_limit(limit);
        let mut total = 0;
        let mut selected = Vec::new();
        self.query.visit_unresolved(
            source_key,
            reason.as_ref(),
            relation.as_ref(),
            path.as_deref(),
            |index, source| {
                total += 1;
                if selected.len() < limit {
                    selected.push((index, source));
                }
            },
        )?;
        let items = selected
            .into_iter()
            .map(|(index, source)| Ok(unresolved_value(&self.query.unresolved(index)?, source)))
            .collect::<Result<Vec<_>, RequestFailure>>()?;
        Ok(json!({
            "count": total,
            "returned": items.len(),
            "truncated": total > items.len(),
            "unresolved": items,
        }))
    }
}

fn node_list(
    snapshot: &ProtocolSnapshot,
    matches: Vec<NodeId>,
    limit: Option<usize>,
) -> Result<Value, RequestFailure> {
    node_list_page(snapshot, matches, None, limit)
}

fn node_list_page(
    snapshot: &ProtocolSnapshot,
    matches: Vec<NodeId>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Value, RequestFailure> {
    let total = matches.len();
    let offset = offset.unwrap_or(0).min(total);
    let limit = bounded_limit(limit);
    let nodes = matches
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|id| {
            let entry = snapshot.entry(id)?.expect("indexed node exists");
            Ok(node_value(&entry))
        })
        .collect::<Result<Vec<_>, RequestFailure>>()?;
    let next_offset = offset + nodes.len();
    let truncated = next_offset < total;
    Ok(json!({
        "count": total,
        "offset": offset,
        "returned": nodes.len(),
        "truncated": truncated,
        "next_offset": truncated.then_some(next_offset),
        "nodes": nodes,
    }))
}

fn intersect_sorted_ids(left: &[NodeId], right: &[NodeId]) -> Vec<NodeId> {
    let mut matches = Vec::new();
    let mut left_index = 0;
    let mut right_index = 0;
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => {
                matches.push(left[left_index]);
                left_index += 1;
                right_index += 1;
            }
        }
    }
    matches
}

fn bounded_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT)
}

fn normalize_optional_path(path: Option<&str>) -> Result<Option<String>, RequestFailure> {
    path.map(|path| {
        normalize_repository_path(path)
            .map_err(|error| RequestFailure::new("invalid_path", error.to_string()))
    })
    .transpose()
}

fn parse_kind(kind: Option<&str>) -> Result<Option<NodeKind>, RequestFailure> {
    kind.map(|kind| {
        NodeKind::parse(kind).ok_or_else(|| {
            RequestFailure::new("invalid_node_kind", format!("unknown node kind '{kind}'"))
        })
    })
    .transpose()
}

fn parse_relation(relation: Option<&str>) -> Result<Option<RelationKind>, RequestFailure> {
    relation
        .map(|relation| {
            RelationKind::parse(relation).ok_or_else(|| {
                RequestFailure::new("invalid_relation", format!("unknown relation '{relation}'"))
            })
        })
        .transpose()
}

fn parse_reason(reason: Option<&str>) -> Result<Option<UnresolvedReason>, RequestFailure> {
    reason
        .map(|reason| {
            UnresolvedReason::parse(reason).ok_or_else(|| {
                RequestFailure::new(
                    "invalid_unresolved_reason",
                    format!("unknown unresolved reason '{reason}'"),
                )
            })
        })
        .transpose()
}
