use serde_json::{Value, json};

use super::session::{ProtocolSnapshot, RequestFailure};
use super::traversal::{
    bounded_depth, bounded_path_limit, bounded_paths, call_relations, parse_relations, path_value,
    require_node, shortest_path,
};

impl ProtocolSnapshot {
    pub(crate) fn paths(
        &self,
        from_node_id: u32,
        to_node_id: u32,
        relations: Option<&[String]>,
        max_depth: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let start = require_node(self, from_node_id)?;
        let target = require_node(self, to_node_id)?;
        let allowed = parse_relations(relations)?;
        let max_depth = bounded_depth(max_depth);
        let limit = bounded_path_limit(limit);
        let (paths, truncated) = bounded_paths(self, start, target, allowed, max_depth, limit)?;
        let values = paths
            .iter()
            .map(|(nodes, relations)| path_value(self, nodes, relations))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({
            "from_node_id": from_node_id,
            "to_node_id": to_node_id,
            "max_depth": max_depth,
            "count": values.len(),
            "truncated": truncated,
            "paths": values,
        }))
    }

    pub(crate) fn shortest_call_chain(
        &self,
        from_node_id: u32,
        to_node_id: u32,
        include_possible: bool,
        max_depth: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let start = require_node(self, from_node_id)?;
        let target = require_node(self, to_node_id)?;
        let max_depth = bounded_depth(max_depth);
        let allowed = call_relations(include_possible);
        let chain = shortest_path(self, start, target, Some(allowed), max_depth)?;
        let value = chain
            .as_ref()
            .map(|(nodes, relations)| path_value(self, nodes, relations))
            .transpose()?;
        Ok(json!({
            "from_node_id": from_node_id,
            "to_node_id": to_node_id,
            "include_possible": include_possible,
            "max_depth": max_depth,
            "found": value.is_some(),
            "chain": value,
        }))
    }
}
