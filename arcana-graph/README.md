# arcana-graph

`arcana-graph` is the repository-agnostic graph kernel extracted from Arcana.

It owns:

- dense graph primitives (`NodeId`, `EdgeKind`, `Edge`, `GraphDataset`);
- canonical dataset validation and stable checksums;
- immutable packed forward/reverse adjacency storage;
- graph snapshots, edge overlays, visible reads, and compaction;
- generic BFS/reachability, shortest-path, bounded simple-path, and connected-component traversal.

It intentionally does **not** own repository facts, source paths or spans, Lexicon ingestion, Arcana's relationship vocabulary, repository snapshots, JSON protocol shapes, vectors, or CLI behavior. Consumers map their own stable identities and semantic relationship types onto the dense topology primitives.

Arcana depends on this crate through a local path dependency and preserves its existing `storage` and `snapshot` module paths as compatibility re-exports.

## Verification

```text
cargo fmt --manifest-path arcana-graph/Cargo.toml -- --check
cargo test --manifest-path arcana-graph/Cargo.toml --all-targets
```
