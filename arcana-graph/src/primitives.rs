//! Dense graph primitives and logical datasets.

/// A dense graph node identifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct NodeId(pub u32);

/// The kind of relationship represented by an edge.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct EdgeKind(pub u16);

/// A directed relationship between two distinct nodes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
}

/// A logical graph dataset with dense node-count metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphDataset {
    pub node_count: u32,
    pub edges: Vec<Edge>,
}
