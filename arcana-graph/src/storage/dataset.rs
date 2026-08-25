use crate::{Edge, GraphDataset};

use super::DatasetError;
use super::format::StableHasher;

pub(crate) fn canonical_edges(dataset: &GraphDataset) -> Result<Vec<Edge>, DatasetError> {
    let mut edges = dataset.edges.clone();
    edges.sort_unstable();

    for edge in &edges {
        if edge.source.0 >= dataset.node_count || edge.target.0 >= dataset.node_count {
            return Err(DatasetError::EndpointOutOfRange {
                edge: *edge,
                node_count: dataset.node_count,
            });
        }
    }

    if let Some(pair) = edges.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(DatasetError::DuplicateEdge { edge: pair[0] });
    }

    Ok(edges)
}

pub fn dataset_checksum(node_count: u32, edges: &[Edge]) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.update(&node_count.to_le_bytes());
    hasher.update(&(edges.len() as u64).to_le_bytes());
    for edge in edges {
        hasher.update(&edge.source.0.to_le_bytes());
        hasher.update(&edge.target.0.to_le_bytes());
        hasher.update(&edge.kind.0.to_le_bytes());
    }
    hasher.finish()
}
