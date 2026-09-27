use std::hint::black_box;
use std::path::Path;

use super::current_metadata;
use super::snapshot_compact::{load_and_write_compact, load_compact};
use crate::repository::compile_compact_repository_graph;
use crate::storage::write_packed;

#[test]
#[ignore = "manual release-mode compact Lexicon ingestion benchmark"]
fn benchmark_current_snapshot_compact_load() {
    let root = std::env::var("ARCANA_COMPACT_BENCH_ROOT")
        .expect("ARCANA_COMPACT_BENCH_ROOT must name a Lexicon state root");
    let metadata = current_metadata(&root).unwrap();
    let snapshot = load_compact(&root, metadata.id()).unwrap();
    assert!(snapshot.direct_v2);
    eprintln!(
        "compact-lexicon nodes={} edges={} unresolved={} strings={} warnings={}",
        snapshot.repository.nodes.len(),
        snapshot.repository.edges.len(),
        snapshot.repository.unresolved.len(),
        snapshot.repository.strings.len(),
        snapshot.compatibility_warnings.len(),
    );
    black_box(snapshot);
}

#[test]
#[ignore = "manual release-mode compact Lexicon writer benchmark"]
fn benchmark_current_snapshot_compact_write() {
    let root = std::env::var("ARCANA_COMPACT_BENCH_ROOT")
        .expect("ARCANA_COMPACT_BENCH_ROOT must name a Lexicon state root");
    let output = std::env::var("ARCANA_COMPACT_BENCH_OUTPUT")
        .expect("ARCANA_COMPACT_BENCH_OUTPUT must name a new repository.arcana path");
    let metadata = current_metadata(&root).unwrap();
    let (snapshot, write) =
        load_and_write_compact(&root, metadata.id(), Path::new(&output)).unwrap();
    eprintln!(
        "compact-store nodes={} edges={} unresolved={} strings={} file_len={} warnings={}",
        snapshot.repository.nodes.len(),
        snapshot.repository.edges.len(),
        snapshot.repository.unresolved.len(),
        snapshot.repository.strings.len(),
        write.header.file_len,
        snapshot.compatibility_warnings.len(),
    );
    black_box((snapshot, write));
}

#[test]
#[ignore = "manual release-mode compact Lexicon graph benchmark"]
fn benchmark_current_snapshot_compact_graph() {
    let root = std::env::var("ARCANA_COMPACT_BENCH_ROOT")
        .expect("ARCANA_COMPACT_BENCH_ROOT must name a Lexicon state root");
    let output = std::env::var("ARCANA_COMPACT_GRAPH_OUTPUT")
        .expect("ARCANA_COMPACT_GRAPH_OUTPUT must name a new graph.arcana path");
    let metadata = current_metadata(&root).unwrap();
    let snapshot = load_compact(&root, metadata.id()).unwrap();
    let graph = compile_compact_repository_graph(&snapshot.repository).unwrap();
    let write = write_packed(Path::new(&output), &graph.dataset).unwrap();
    eprintln!(
        "compact-graph nodes={} fact_edges={} graph_edges={} unresolved={} file_len={} warnings={}",
        snapshot.repository.nodes.len(),
        snapshot.repository.edges.len(),
        graph.dataset.edges.len(),
        snapshot.repository.unresolved.len(),
        write.file_len,
        snapshot.compatibility_warnings.len(),
    );
    black_box((snapshot, graph, write));
}
