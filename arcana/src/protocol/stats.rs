use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::repository::edge_kind_to_relation;
use crate::synthetic::NodeId;

use super::session::{ProtocolSnapshot, RequestFailure};

impl ProtocolSnapshot {
    pub(crate) fn stats(&self) -> Result<Value, RequestFailure> {
        let node_kinds = self.query.node_kind_counts()?;

        let mut relations = BTreeMap::<String, u64>::new();
        for node in 0..self.query.graph().node_count() {
            for neighbor in self
                .query
                .graph()
                .forward_neighbors(NodeId(node))
                .map_err(|error| RequestFailure::new("query_failed", error.to_string()))?
            {
                let relation = edge_kind_to_relation(neighbor.kind).ok_or_else(|| {
                    RequestFailure::new(
                        "corrupt_graph",
                        format!("unknown edge kind {}", neighbor.kind.0),
                    )
                })?;
                *relations.entry(relation.as_str().to_owned()).or_default() += 1;
            }
        }

        let (unresolved_reasons, unresolved_calls) = self.query.unresolved_statistics()?;
        let resolved_call_relationships = relations.get("calls").copied().unwrap_or(0);
        let possible_call_relationships = relations.get("possible-calls").copied().unwrap_or(0);
        let conversion_relationships = relations.get("converts-to").copied().unwrap_or(0);
        let runtime_confirmed_relationships = relations.get("observed-calls").copied().unwrap_or(0);

        Ok(json!({
            "node_count": self.query.graph().node_count(),
            "edge_count": self.query.graph().edge_count(),
            "unresolved_count": self.query.manifest().unresolved_count,
            "dataset_checksum": format!("{:016x}", self.query.graph().dataset_checksum()),
            "nodes_by_kind": node_kinds,
            "edges_by_relation": relations,
            "unresolved_by_reason": unresolved_reasons,
            "call_resolution": {
                "resolved_unique_relationships": resolved_call_relationships,
                "possible_call_relationships": possible_call_relationships,
                "conversion_relationships": conversion_relationships,
                "runtime_confirmed_relationships": runtime_confirmed_relationships,
                "unresolved_references": unresolved_calls,
                "coverage_available": false,
                "coverage": Value::Null,
                "coverage_unavailable_reason":
                    "resolved call sites are deduplicated into graph relationships",
            },
        }))
    }
}
