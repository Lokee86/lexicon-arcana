use std::collections::VecDeque;

use serde_json::{Value, json};

use crate::repository::{RelationKind, edge_kind_to_relation};
use crate::synthetic::NodeId;

use super::request::QueryDirection;
use super::response::node_value;
use super::session::{ProtocolSnapshot, RequestFailure};

pub(crate) const DEFAULT_DEPTH: usize = 12;
pub(crate) const MAX_DEPTH: usize = 64;
pub(crate) const DEFAULT_RESULT_LIMIT: usize = 1_000;
pub(crate) const MAX_RESULT_LIMIT: usize = 10_000;
pub(crate) const DEFAULT_PATH_LIMIT: usize = 100;
pub(crate) const MAX_PATH_LIMIT: usize = 1_000;

pub(crate) type GraphPath = (Vec<NodeId>, Vec<RelationKind>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationMask(u32);

impl RelationMask {
    const fn empty() -> Self {
        Self(0)
    }

    fn insert(&mut self, relation: &RelationKind) {
        self.0 |= relation_bit(relation);
    }

    pub(crate) fn contains(self, relation: &RelationKind) -> bool {
        self.0 & relation_bit(relation) != 0
    }

    pub(crate) fn relation_names(self) -> Vec<&'static str> {
        RELATION_ORDER
            .iter()
            .filter(|relation| self.contains(relation))
            .map(RelationKind::as_str)
            .collect()
    }
}

const RELATION_ORDER: [RelationKind; 32] = [
    RelationKind::Contains,
    RelationKind::Defines,
    RelationKind::References,
    RelationKind::Imports,
    RelationKind::Calls,
    RelationKind::PossibleCalls,
    RelationKind::PassesTo,
    RelationKind::ObservedCalls,
    RelationKind::ConvertsTo,
    RelationKind::Implements,
    RelationKind::Extends,
    RelationKind::UsesTrait,
    RelationKind::Overrides,
    RelationKind::Reads,
    RelationKind::Writes,
    RelationKind::Annotates,
    RelationKind::Includes,
    RelationKind::DependsOn,
    RelationKind::RoutesTo,
    RelationKind::CommunicatesWith,
    RelationKind::Tests,
    RelationKind::Documents,
    RelationKind::Generates,
    RelationKind::SimilarTo,
    RelationKind::CallsEndpoint,
    RelationKind::HandledBy,
    RelationKind::Publishes,
    RelationKind::Consumes,
    RelationKind::ReadsConfig,
    RelationKind::InvokesProcess,
    RelationKind::ProducesMessage,
    RelationKind::ConsumesMessage,
];

pub(crate) fn bounded_depth(depth: Option<usize>) -> usize {
    depth.unwrap_or(DEFAULT_DEPTH).min(MAX_DEPTH)
}

pub(crate) fn bounded_result_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_RESULT_LIMIT).min(MAX_RESULT_LIMIT)
}

pub(crate) fn bounded_path_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_PATH_LIMIT).min(MAX_PATH_LIMIT)
}

pub(crate) fn require_node(
    snapshot: &ProtocolSnapshot,
    value: u32,
) -> Result<NodeId, RequestFailure> {
    let node = NodeId(value);
    snapshot
        .entry(node)?
        .map(|_| node)
        .ok_or_else(|| RequestFailure::new("unknown_node", format!("node {value} does not exist")))
}

pub(crate) fn require_entries(
    snapshot: &ProtocolSnapshot,
    values: &[u32],
) -> Result<Vec<NodeId>, RequestFailure> {
    if values.is_empty() {
        return Err(RequestFailure::new(
            "missing_entry_points",
            "entry_node_ids must contain at least one node",
        ));
    }
    let mut entries = values
        .iter()
        .map(|value| require_node(snapshot, *value))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_unstable();
    entries.dedup();
    Ok(entries)
}

pub(crate) fn parse_relations(
    values: Option<&[String]>,
) -> Result<Option<RelationMask>, RequestFailure> {
    let Some(values) = values else {
        return Ok(None);
    };
    let mut relations = RelationMask::empty();
    for value in values {
        let relation = RelationKind::parse(value).ok_or_else(|| {
            RequestFailure::new("invalid_relation", format!("unknown relation '{value}'"))
        })?;
        relations.insert(&relation);
    }
    Ok(Some(relations))
}

pub(crate) fn call_relations(include_possible: bool) -> RelationMask {
    let mut relations = RelationMask::empty();
    relations.insert(&RelationKind::Calls);
    relations.insert(&RelationKind::ObservedCalls);
    relations.insert(&RelationKind::CallsEndpoint);
    relations.insert(&RelationKind::HandledBy);
    relations.insert(&RelationKind::Publishes);
    relations.insert(&RelationKind::Consumes);
    relations.insert(&RelationKind::InvokesProcess);
    relations.insert(&RelationKind::ProducesMessage);
    relations.insert(&RelationKind::ConsumesMessage);
    if include_possible {
        relations.insert(&RelationKind::PossibleCalls);
    }
    relations
}

pub(crate) fn impact_relations() -> RelationMask {
    let mut relations = RelationMask::empty();
    relations.insert(&RelationKind::Calls);
    relations.insert(&RelationKind::PossibleCalls);
    relations.insert(&RelationKind::PassesTo);
    relations.insert(&RelationKind::ObservedCalls);
    relations.insert(&RelationKind::References);
    relations.insert(&RelationKind::CallsEndpoint);
    relations.insert(&RelationKind::HandledBy);
    relations.insert(&RelationKind::Publishes);
    relations.insert(&RelationKind::Consumes);
    relations.insert(&RelationKind::ReadsConfig);
    relations.insert(&RelationKind::InvokesProcess);
    relations.insert(&RelationKind::ProducesMessage);
    relations.insert(&RelationKind::ConsumesMessage);
    relations
}

pub(crate) fn architecture_relations() -> RelationMask {
    let mut relations = RelationMask::empty();
    for relation in [
        RelationKind::Imports,
        RelationKind::Calls,
        RelationKind::PossibleCalls,
        RelationKind::ObservedCalls,
        RelationKind::Implements,
        RelationKind::Extends,
        RelationKind::UsesTrait,
        RelationKind::Overrides,
        RelationKind::Includes,
        RelationKind::DependsOn,
        RelationKind::RoutesTo,
        RelationKind::CommunicatesWith,
        RelationKind::CallsEndpoint,
        RelationKind::HandledBy,
        RelationKind::Publishes,
        RelationKind::Consumes,
        RelationKind::ReadsConfig,
        RelationKind::InvokesProcess,
        RelationKind::ProducesMessage,
        RelationKind::ConsumesMessage,
    ] {
        relations.insert(&relation);
    }
    relations
}

pub(crate) fn graph_neighbors(
    snapshot: &ProtocolSnapshot,
    node: NodeId,
    direction: QueryDirection,
    allowed: Option<RelationMask>,
) -> Result<Vec<(NodeId, RelationKind)>, RequestFailure> {
    let neighbors = match direction {
        QueryDirection::Outgoing => snapshot.query.graph().forward_neighbors_iter(node),
        QueryDirection::Incoming => snapshot.query.graph().reverse_neighbors_iter(node),
    }
    .map_err(|error| RequestFailure::new("query_failed", error.to_string()))?;

    let mut result = Vec::new();
    for neighbor in neighbors {
        let relation = edge_kind_to_relation(neighbor.kind).ok_or_else(|| {
            RequestFailure::new(
                "corrupt_graph",
                format!("unknown edge kind {}", neighbor.kind.0),
            )
        })?;
        if allowed.is_none_or(|allowed| allowed.contains(&relation)) {
            result.push((neighbor.node, relation));
        }
    }
    result.sort_unstable_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    Ok(result)
}

pub(crate) fn bfs_distances(
    snapshot: &ProtocolSnapshot,
    starts: &[NodeId],
    direction: QueryDirection,
    allowed: Option<RelationMask>,
    max_depth: usize,
) -> Result<Vec<Option<usize>>, RequestFailure> {
    let mut distances = vec![None; snapshot.query.graph().node_count() as usize];
    let mut queue = VecDeque::new();
    for start in starts {
        distances[start.0 as usize] = Some(0);
        queue.push_back(*start);
    }
    while let Some(node) = queue.pop_front() {
        let depth = distances[node.0 as usize].expect("queued nodes always have a distance");
        if depth >= max_depth {
            continue;
        }
        for (neighbor, _) in graph_neighbors(snapshot, node, direction, allowed)? {
            let index = neighbor.0 as usize;
            if distances[index].is_none() {
                distances[index] = Some(depth + 1);
                queue.push_back(neighbor);
            }
        }
    }
    Ok(distances)
}

pub(crate) fn shortest_path(
    snapshot: &ProtocolSnapshot,
    start: NodeId,
    target: NodeId,
    allowed: Option<RelationMask>,
    max_depth: usize,
) -> Result<Option<GraphPath>, RequestFailure> {
    if start == target {
        return Ok(Some((vec![start], Vec::new())));
    }
    let mut depth = vec![None; snapshot.query.graph().node_count() as usize];
    let mut parent = vec![None; snapshot.query.graph().node_count() as usize];
    depth[start.0 as usize] = Some(0);
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        let current_depth = depth[node.0 as usize].expect("queued nodes always have a distance");
        if current_depth >= max_depth {
            continue;
        }
        for (neighbor, relation) in
            graph_neighbors(snapshot, node, QueryDirection::Outgoing, allowed)?
        {
            let index = neighbor.0 as usize;
            if depth[index].is_some() {
                continue;
            }
            depth[index] = Some(current_depth + 1);
            parent[index] = Some((node, relation));
            if neighbor == target {
                return Ok(Some(reconstruct_path(start, target, &parent)));
            }
            queue.push_back(neighbor);
        }
    }
    Ok(None)
}

fn reconstruct_path(
    start: NodeId,
    target: NodeId,
    parent: &[Option<(NodeId, RelationKind)>],
) -> (Vec<NodeId>, Vec<RelationKind>) {
    let mut nodes = vec![target];
    let mut relations = Vec::new();
    let mut current = target;
    while current != start {
        let (previous, relation) = parent[current.0 as usize]
            .clone()
            .expect("target must have a reconstructed parent chain");
        nodes.push(previous);
        relations.push(relation);
        current = previous;
    }
    nodes.reverse();
    relations.reverse();
    (nodes, relations)
}

pub(crate) fn path_value(
    snapshot: &ProtocolSnapshot,
    nodes: &[NodeId],
    relations: &[RelationKind],
) -> Result<Value, RequestFailure> {
    let node_values = nodes
        .iter()
        .map(|node| {
            snapshot
                .entry(*node)?
                .map(|entry| node_value(&entry))
                .ok_or_else(|| {
                    RequestFailure::new(
                        "invalid_snapshot",
                        format!("missing catalogue node {}", node.0),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "depth": relations.len(),
        "nodes": node_values,
        "relations": relations.iter().map(RelationKind::as_str).collect::<Vec<_>>(),
    }))
}

pub(crate) fn related_values(
    snapshot: &ProtocolSnapshot,
    values: &[(NodeId, RelationKind)],
) -> Result<Vec<Value>, RequestFailure> {
    values
        .iter()
        .map(|(node, relation)| {
            let entry = snapshot.entry(*node)?.ok_or_else(|| {
                RequestFailure::new(
                    "invalid_snapshot",
                    format!("missing catalogue node {}", node.0),
                )
            })?;
            Ok(json!({"relation": relation.as_str(), "node": node_value(&entry)}))
        })
        .collect()
}

#[inline]
fn relation_bit(relation: &RelationKind) -> u32 {
    match relation {
        RelationKind::Contains => 1 << 0,
        RelationKind::Defines => 1 << 1,
        RelationKind::References => 1 << 2,
        RelationKind::Imports => 1 << 3,
        RelationKind::Calls => 1 << 4,
        RelationKind::PossibleCalls => 1 << 5,
        RelationKind::ConvertsTo => 1 << 6,
        RelationKind::Implements => 1 << 7,
        RelationKind::Extends => 1 << 8,
        RelationKind::UsesTrait => 1 << 9,
        RelationKind::Overrides => 1 << 10,
        RelationKind::Reads => 1 << 11,
        RelationKind::Writes => 1 << 12,
        RelationKind::Annotates => 1 << 13,
        RelationKind::Includes => 1 << 14,
        RelationKind::DependsOn => 1 << 15,
        RelationKind::Tests => 1 << 16,
        RelationKind::Documents => 1 << 17,
        RelationKind::Generates => 1 << 18,
        RelationKind::PassesTo => 1 << 19,
        RelationKind::ObservedCalls => 1 << 20,
        RelationKind::RoutesTo => 1 << 21,
        RelationKind::CommunicatesWith => 1 << 22,
        RelationKind::SimilarTo => 1 << 23,
        RelationKind::CallsEndpoint => 1 << 24,
        RelationKind::HandledBy => 1 << 25,
        RelationKind::Publishes => 1 << 26,
        RelationKind::Consumes => 1 << 27,
        RelationKind::ReadsConfig => 1 << 28,
        RelationKind::InvokesProcess => 1 << 29,
        RelationKind::ProducesMessage => 1 << 30,
        RelationKind::ConsumesMessage => 1 << 31,
    }
}
