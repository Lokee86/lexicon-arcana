use super::response::node_value;
use super::session::{ProtocolSnapshot, RequestFailure};
use crate::repository::{CatalogueEntry, NodeKey};
use crate::synthetic::NodeId;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

impl ProtocolSnapshot {
    pub(crate) fn diff_snapshot(
        &self,
        other_path: &Path,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let other = Self::open(other_path)
            .map_err(|error| RequestFailure::new("snapshot_open_failed", error.to_string()))?;
        let limit = limit.unwrap_or(1_000).min(10_000);
        let mut counts = [0usize; 4];
        let mut values: [Vec<Value>; 4] = std::array::from_fn(|_| Vec::new());
        let mut current = self.query.entries();
        let mut previous = other.query.entries();
        let mut left = current.next().transpose()?;
        let mut right = previous.next().transpose()?;
        let identical = self.query.manifest().repository_store_checksum
            == other.query.manifest().repository_store_checksum
            && self.query.graph().dataset_checksum() == other.query.graph().dataset_checksum();
        while !identical && (left.is_some() || right.is_some()) {
            match (&left, &right) {
                (Some(l), Some(r)) if l.fact.key == r.fact.key => {
                    if l.fact != r.fact {
                        record(&mut counts, &mut values, 2, l, limit);
                    }
                    if logical_outgoing(self, l.node_id)? != logical_outgoing(&other, r.node_id)? {
                        record(&mut counts, &mut values, 3, l, limit);
                    }
                    left = current.next().transpose()?;
                    right = previous.next().transpose()?;
                }
                (Some(l), Some(r)) if l.fact.key > r.fact.key => {
                    record(&mut counts, &mut values, 1, r, limit);
                    right = previous.next().transpose()?;
                }
                (Some(l), _) => {
                    record(&mut counts, &mut values, 0, l, limit);
                    left = current.next().transpose()?;
                }
                (_, Some(r)) => {
                    record(&mut counts, &mut values, 1, r, limit);
                    right = previous.next().transpose()?;
                }
                _ => break,
            }
        }
        let graph_changed = counts[0] != 0 || counts[1] != 0 || counts[3] != 0;
        Ok(json!({
            "current_snapshot": self.root.display().to_string(), "other_snapshot": other.root.display().to_string(),
            "current_checksum": format!("{:016x}", self.query.graph().dataset_checksum()),
            "other_checksum": format!("{:016x}", other.query.graph().dataset_checksum()),
            "snapshot_changed": graph_changed || counts[2] != 0,
            "graph_changed": graph_changed,
            "packed_checksum_changed": self.query.graph().dataset_checksum() != other.query.graph().dataset_checksum(),
            "counts": {"added": counts[0], "removed": counts[1], "metadata_changed": counts[2], "relationship_changed": counts[3]},
            "truncated": counts.iter().any(|count| *count > limit),
            "nodes": {"added": values[0], "removed": values[1], "metadata_changed": values[2], "relationship_changed": values[3]},
        }))
    }
}
fn record(
    counts: &mut [usize; 4],
    values: &mut [Vec<Value>; 4],
    category: usize,
    entry: &CatalogueEntry,
    limit: usize,
) {
    counts[category] += 1;
    if values[category].len() < limit {
        values[category].push(node_value(entry));
    }
}
fn logical_outgoing(
    snapshot: &ProtocolSnapshot,
    id: NodeId,
) -> Result<BTreeSet<(NodeKey, u16)>, RequestFailure> {
    snapshot
        .query
        .graph()
        .forward_neighbors_iter(id)
        .map_err(|error| RequestFailure::new("query_failed", error.to_string()))?
        .map(|neighbor| {
            let entry = snapshot.entry(neighbor.node)?.ok_or_else(|| {
                RequestFailure::new("invalid_snapshot", "graph node metadata absent")
            })?;
            Ok((entry.fact.key, neighbor.kind.0))
        })
        .collect()
}
