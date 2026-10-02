use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::repository::{edge_kind_to_relation, normalize_repository_path};
use crate::synthetic::NodeId;

use super::response::node_value;
use super::session::{ProtocolSnapshot, RequestFailure};

const DEFAULT_EXPORT_LIMIT: usize = 1_000;
const MAX_EXPORT_LIMIT: usize = 100_000;

impl ProtocolSnapshot {
    pub(crate) fn export_graph(
        &self,
        path_prefix: Option<&str>,
        offset: Option<usize>,
        limit: Option<usize>,
        pinned_node_ids: &[u32],
    ) -> Result<Value, RequestFailure> {
        let path_prefix = path_prefix
            .map(|path| {
                normalize_repository_path(path)
                    .map_err(|error| RequestFailure::new("invalid_path", error.to_string()))
            })
            .transpose()?;
        let matches = match path_prefix {
            Some(prefix) => self
                .query
                .node_ids_by_path_prefix(&prefix)
                .map_err(|error| RequestFailure::new("invalid_path", error.to_string()))?,
            None => (0..self.query.graph().node_count()).map(NodeId).collect(),
        };
        let count = matches.len();
        let offset = offset.unwrap_or(0).min(count);
        let mut page = matches
            .into_iter()
            .skip(offset)
            .take(bounded_export_limit(limit))
            .collect::<Vec<_>>();
        let page_returned = page.len();
        let mut page_nodes = page.iter().copied().collect::<BTreeSet<_>>();
        let mut pinned_returned = 0;
        for node_id in pinned_node_ids.iter().copied().map(NodeId) {
            if self.entry(node_id)?.is_none() {
                return Err(RequestFailure::new(
                    "unknown_node",
                    format!("pinned node {} does not exist", node_id.0),
                ));
            }
            if page_nodes.insert(node_id) {
                page.push(node_id);
                pinned_returned += 1;
            }
        }
        let mut page_edges = Vec::new();
        for source in &page {
            let neighbors = self
                .query
                .graph()
                .forward_neighbors_iter(*source)
                .map_err(|error| RequestFailure::new("query_failed", error.to_string()))?;
            for neighbor in neighbors {
                if !page_nodes.contains(&neighbor.node) {
                    continue;
                }
                let relation = edge_kind_to_relation(neighbor.kind).ok_or_else(|| {
                    RequestFailure::new(
                        "corrupt_graph",
                        format!("unknown edge kind {}", neighbor.kind.0),
                    )
                })?;
                page_edges.push((source.0, neighbor.node.0, relation.as_str()));
            }
        }
        page_edges.sort_unstable();
        let edges = page_edges
            .into_iter()
            .map(|(source_node_id, target_node_id, relation)| {
                json!({
                    "id": format!("{source_node_id}:{relation}:{target_node_id}"),
                    "source_node_id": source_node_id,
                    "target_node_id": target_node_id,
                    "relation": relation,
                })
            })
            .collect::<Vec<_>>();
        let nodes = page
            .iter()
            .map(|id| {
                let entry = self.entry(*id)?.expect("validated page node");
                Ok(node_value(&entry))
            })
            .collect::<Result<Vec<_>, RequestFailure>>()?;
        let next_offset = offset + page_returned;
        let truncated = next_offset < count;
        Ok(json!({
            "count": count,
            "offset": offset,
            "returned": nodes.len(),
            "page_returned": page_returned,
            "pinned_returned": pinned_returned,
            "truncated": truncated,
            "next_offset": truncated.then_some(next_offset),
            "nodes": nodes,
            "edges": edges,
        }))
    }
}

fn bounded_export_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_EXPORT_LIMIT).min(MAX_EXPORT_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::{MAX_EXPORT_LIMIT, bounded_export_limit};

    #[test]
    fn caps_export_pages_at_one_hundred_thousand_nodes() {
        assert_eq!(
            bounded_export_limit(Some(MAX_EXPORT_LIMIT + 1)),
            MAX_EXPORT_LIMIT
        );
    }
}
