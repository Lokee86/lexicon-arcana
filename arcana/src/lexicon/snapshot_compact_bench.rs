use std::hint::black_box;

use super::current_metadata;
use super::snapshot_compact::load_compact;

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
