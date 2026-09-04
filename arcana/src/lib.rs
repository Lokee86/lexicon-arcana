//! Reusable library boundary for Arcana.
//!
//! Synthetic workloads and future graph-storage implementations are exposed
//! from this crate rather than coupled to the `arcana` command-line binary.

pub mod benchmark;
pub mod lexicon;
pub mod protocol;
pub mod repository;
pub mod snapshot;
pub mod storage;
pub mod synthetic;
pub mod traversal;
pub mod vector;

pub use synthetic::{Edge, EdgeKind, GraphDataset, NodeId};

#[cfg(test)]
mod traversal_tests;

/// Product name presented by the Arcana library and CLI.
pub const PROJECT_NAME: &str = "Arcana";

/// Package version supplied by the release workflow or, for standalone Cargo
/// builds, by the package manifest.
pub const PROJECT_VERSION: &str = match option_env!("ARCANA_RELEASE_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

/// Returns the short project description used by integrations.
pub const fn about() -> &'static str {
    "independent repository-graph foundation"
}

#[cfg(test)]
mod tests {
    use super::{PROJECT_NAME, PROJECT_VERSION, about};

    #[test]
    fn exposes_stable_project_metadata() {
        assert_eq!(PROJECT_NAME, "Arcana");
        assert!(!PROJECT_VERSION.is_empty());
        assert_eq!(about(), "independent repository-graph foundation");
    }
}
