use serde_json::{Value, json};

use crate::repository::RelationKind;
use crate::synthetic::NodeId;

use super::request::QueryDirection;
use super::session::{ProtocolSnapshot, RequestFailure};
use super::traversal::{
    bounded_depth, bounded_path_limit, call_relations, graph_neighbors, parse_relations,
    path_value, require_node, shortest_path,
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
        let mut state = PathSearch {
            snapshot: self,
            target,
            allowed,
            max_depth,
            limit,
            paths: Vec::new(),
            truncated: false,
            nodes: vec![start],
            relations: Vec::new(),
            visited: {
                let mut visited = vec![false; self.query.graph().node_count() as usize];
                visited[start.0 as usize] = true;
                visited
            },
        };
        state.walk(start)?;
        let values = state
            .paths
            .iter()
            .map(|(nodes, relations)| path_value(self, nodes, relations))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({
            "from_node_id": from_node_id,
            "to_node_id": to_node_id,
            "max_depth": max_depth,
            "count": values.len(),
            "truncated": state.truncated,
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

struct PathSearch<'a> {
    snapshot: &'a ProtocolSnapshot,
    target: NodeId,
    allowed: Option<super::traversal::RelationMask>,
    max_depth: usize,
    limit: usize,
    paths: Vec<(Vec<NodeId>, Vec<RelationKind>)>,
    truncated: bool,
    nodes: Vec<NodeId>,
    relations: Vec<RelationKind>,
    visited: Vec<bool>,
}

impl PathSearch<'_> {
    fn walk(&mut self, current: NodeId) -> Result<(), RequestFailure> {
        if self.paths.len() >= self.limit {
            self.truncated = true;
            return Ok(());
        }
        if current == self.target {
            self.paths
                .push((self.nodes.clone(), self.relations.clone()));
            return Ok(());
        }
        if self.relations.len() >= self.max_depth {
            return Ok(());
        }
        for (neighbor, relation) in graph_neighbors(
            self.snapshot,
            current,
            QueryDirection::Outgoing,
            self.allowed,
        )? {
            if self.paths.len() >= self.limit {
                self.truncated = true;
                break;
            }
            let index = neighbor.0 as usize;
            if self.visited[index] {
                continue;
            }
            self.visited[index] = true;
            self.nodes.push(neighbor);
            self.relations.push(relation);
            self.walk(neighbor)?;
            self.relations.pop();
            self.nodes.pop();
            self.visited[index] = false;
        }
        Ok(())
    }
}
