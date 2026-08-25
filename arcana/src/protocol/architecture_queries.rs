use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;

use arcana_graph::traversal::connected_components as shared_connected_components;

use serde_json::{Value, json};

use crate::repository::normalize_repository_path;
use crate::synthetic::NodeId;

use super::request::QueryDirection;
use super::response::node_value;
use super::session::{ProtocolSnapshot, RequestFailure};
use super::traversal::{
    architecture_relations, bounded_result_limit, graph_neighbors, parse_relations,
};

const DEFAULT_MIN_COMMUNITY_SIZE: usize = 2;
const REPRESENTATIVE_LIMIT: usize = 5;

#[derive(Debug)]
struct CommunityData {
    nodes: Vec<NodeId>,
    edge_count: usize,
    relation_counts: BTreeMap<&'static str, usize>,
    incoming_boundary_counts: BTreeMap<&'static str, usize>,
    outgoing_boundary_counts: BTreeMap<&'static str, usize>,
    degrees: BTreeMap<NodeId, usize>,
}

impl ProtocolSnapshot {
    pub(crate) fn architecture_summary(
        &self,
        path_prefix: Option<&str>,
        relations: Option<&[String]>,
        min_community_size: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value, RequestFailure> {
        let path_prefix = normalize_prefix(path_prefix)?;
        let relation_mask = parse_relations(relations)?.unwrap_or_else(architecture_relations);
        let selected = selected_nodes(self, path_prefix.as_deref())?;
        let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
        let graph_size = self.graph.node_count() as usize;
        let mut adjacency = vec![BTreeSet::new(); graph_size];
        let mut internal_edges = Vec::new();
        let mut boundary_events = Vec::new();

        for source in &selected {
            for (target, relation) in
                graph_neighbors(self, *source, QueryDirection::Outgoing, Some(relation_mask))?
            {
                if selected_set.contains(&target) {
                    adjacency[source.0 as usize].insert(target);
                    adjacency[target.0 as usize].insert(*source);
                    internal_edges.push((*source, target, relation));
                } else {
                    boundary_events.push((*source, QueryDirection::Outgoing, relation));
                }
            }
            for (source_outside, relation) in
                graph_neighbors(self, *source, QueryDirection::Incoming, Some(relation_mask))?
            {
                if !selected_set.contains(&source_outside) {
                    boundary_events.push((*source, QueryDirection::Incoming, relation));
                }
            }
        }

        let connected = shared_connected_components(self.graph.node_count(), &selected, |node| {
            Ok::<_, Infallible>(adjacency[node.0 as usize].iter().copied().collect())
        })
        .expect("selected adjacency contains only validated graph nodes");
        let component_count = connected.components.len();
        let component_of = connected.component_of;
        let mut communities = connected
            .components
            .into_iter()
            .map(|nodes| CommunityData {
                nodes,
                edge_count: 0,
                relation_counts: BTreeMap::new(),
                incoming_boundary_counts: BTreeMap::new(),
                outgoing_boundary_counts: BTreeMap::new(),
                degrees: BTreeMap::new(),
            })
            .collect::<Vec<_>>();

        for (source, target, relation) in &internal_edges {
            let index = component_of[source.0 as usize]
                .expect("selected nodes always belong to a component");
            let community = &mut communities[index];
            community.edge_count += 1;
            *community
                .relation_counts
                .entry(relation.as_str())
                .or_default() += 1;
            *community.degrees.entry(*source).or_default() += 1;
            if target != source {
                *community.degrees.entry(*target).or_default() += 1;
            }
        }
        let boundary_edge_count = boundary_events.len();
        for (node, direction, relation) in boundary_events {
            let index =
                component_of[node.0 as usize].expect("selected nodes always belong to a component");
            let counts = match direction {
                QueryDirection::Incoming => &mut communities[index].incoming_boundary_counts,
                QueryDirection::Outgoing => &mut communities[index].outgoing_boundary_counts,
            };
            *counts.entry(relation.as_str()).or_default() += 1;
        }

        let min_community_size = min_community_size
            .unwrap_or(DEFAULT_MIN_COMMUNITY_SIZE)
            .max(1);
        let excluded_node_count = communities
            .iter()
            .filter(|community| community.nodes.len() < min_community_size)
            .map(|community| community.nodes.len())
            .sum::<usize>();
        communities.retain(|community| community.nodes.len() >= min_community_size);
        communities.sort_unstable_by(|left, right| {
            right
                .nodes
                .len()
                .cmp(&left.nodes.len())
                .then(right.edge_count.cmp(&left.edge_count))
                .then(left.nodes[0].cmp(&right.nodes[0]))
        });

        let community_count = communities.len();
        let limit = bounded_result_limit(limit);
        let summaries = communities
            .into_iter()
            .take(limit)
            .map(|community| community_value(self, community))
            .collect::<Vec<_>>();
        Ok(json!({
            "path_prefix": path_prefix,
            "relations": relation_mask.relation_names(),
            "node_count": selected.len(),
            "internal_edge_count": internal_edges.len(),
            "boundary_edge_count": boundary_edge_count,
            "component_count": component_count,
            "community_count": community_count,
            "returned": summaries.len(),
            "truncated": community_count > summaries.len(),
            "min_community_size": min_community_size,
            "excluded_node_count": excluded_node_count,
            "communities": summaries,
        }))
    }
}

fn selected_nodes(
    snapshot: &ProtocolSnapshot,
    path_prefix: Option<&str>,
) -> Result<Vec<NodeId>, RequestFailure> {
    match path_prefix {
        Some(prefix) => snapshot
            .catalogue
            .node_ids_by_path_prefix(prefix)
            .map_err(|error| RequestFailure::new("invalid_path", error.to_string())),
        None => Ok(snapshot
            .catalogue
            .entries()
            .iter()
            .map(|entry| entry.node_id)
            .collect()),
    }
}

fn normalize_prefix(path_prefix: Option<&str>) -> Result<Option<String>, RequestFailure> {
    path_prefix
        .map(|path| {
            normalize_repository_path(path)
                .map_err(|error| RequestFailure::new("invalid_path", error.to_string()))
        })
        .transpose()
}

fn community_value(snapshot: &ProtocolSnapshot, community: CommunityData) -> Value {
    let mut representatives = community.nodes.clone();
    representatives.sort_unstable_by(|left, right| {
        community
            .degrees
            .get(right)
            .copied()
            .unwrap_or(0)
            .cmp(&community.degrees.get(left).copied().unwrap_or(0))
            .then(left.cmp(right))
    });
    representatives.truncate(REPRESENTATIVE_LIMIT);

    let mut kind_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut path_counts: BTreeMap<String, usize> = BTreeMap::new();
    for node in &community.nodes {
        let entry = snapshot
            .entry(*node)
            .expect("community nodes originate from the catalogue");
        *kind_counts.entry(entry.fact.kind.as_str()).or_default() += 1;
        *path_counts
            .entry(parent_path(&entry.fact.path).to_owned())
            .or_default() += 1;
    }
    let mut paths = path_counts.into_iter().collect::<Vec<_>>();
    paths.sort_unstable_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));

    json!({
        "community_id": community.nodes[0].0,
        "node_count": community.nodes.len(),
        "edge_count": community.edge_count,
        "relation_counts": community.relation_counts,
        "incoming_boundary_counts": community.incoming_boundary_counts,
        "outgoing_boundary_counts": community.outgoing_boundary_counts,
        "kind_counts": kind_counts,
        "paths": paths.into_iter().map(|(path, node_count)| json!({
            "path": path,
            "node_count": node_count,
        })).collect::<Vec<_>>(),
        "representative_nodes": representatives.into_iter().map(|node| {
            node_value(snapshot.entry(node).expect("representative nodes exist"))
        }).collect::<Vec<_>>(),
    })
}

fn parent_path(path: &str) -> &str {
    path.rsplit_once('/').map_or(".", |(parent, _)| parent)
}
