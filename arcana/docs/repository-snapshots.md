# Repository snapshots and incremental updates

Parent index: [Arcana Documentation](README.md)

## Purpose

This document defines Arcana repository generations, initial import, changed-file updates, immutable publication, overlay behavior, and the node-set rebuild boundary.

## Overview

A repository snapshot binds a verified graph, catalogue, unresolved records, and source facts into one immutable generation. Edge-only changes may use overlays; node-set changes require a packed rebuild.

Arcana repository snapshots bind the graph and its semantic metadata into one verified generation.

A published repository snapshot contains:

- `graph.arcana` — immutable packed adjacency base;
- optional `overlay.arcana` — cumulative edge additions and tombstones relative to the packed base;
- `graph.manifest` — graph counts, checksums, and component paths;
- `catalogue.tsv` — dense node IDs mapped to stable logical identities and metadata;
- `unresolved.tsv` — unresolved references retained for later resolver passes;
- `facts.tsv` — canonical source facts used to reconstruct and incrementally update the generation;
- `repository.manifest` — the final publication record binding every artifact, adapter version, fact schema, and repository identity.

Opening `repository.manifest` verifies every artifact checksum and recompiles `facts.tsv` to confirm that the packed graph, catalogue, and unresolved records represent the same generation.

## Initial import

```text
arcana import-facts \
  --facts repository.tsv \
  --output .arcana/generation-1 \
  --adapter go \
  --adapter-version 1
```

The output directory must not already exist. `repository.manifest` is written last.

## Changed-file update

```text
arcana update-facts \
  --base .arcana/generation-1/repository.manifest \
  --facts rescanned-repository.tsv \
  --changed internal/example/a.go \
  --changed internal/example/b.go \
  --output .arcana/generation-2
```

Arcana partitions existing facts by owning source file, replaces facts owned by the declared paths, recompiles the visible repository, and creates a cumulative overlay relative to the original packed base.

The replacement input is currently a complete adapter fact file. Only facts owned by `--changed` paths are selected from it. This keeps the storage/update boundary ready for adapters that later emit file-scoped fact batches directly.

## Rebuild boundary

Packed node IDs are dense and immutable within a base generation. An overlay can add and remove edges, but it cannot add or remove nodes without changing those IDs.

`update-facts` therefore succeeds when the stable node-key set is unchanged. If declarations are added, removed, or renamed, Arcana returns an explicit rebuild-required error. A later generation should then be produced with `import-facts` or compaction/rebuild tooling.

This rule preserves fast packed traversal and prevents an incremental update from silently invalidating node identities used by consumers.

## Code map

| Snapshot concern | Primary implementation | Related tests |
| --- | --- | --- |
| Repository manifest and publication | `src/repository/repository_snapshot.rs`, `repository_snapshot_format.rs`, `repository_snapshot_validation.rs` | `repository_snapshot_tests.rs` |
| Packed graph manifest | sibling `arcana-graph/src/snapshot/graph.rs`, `manifest.rs`, `manifest_io.rs` | shared graph and manifest tests |
| Overlay format and visible reads | sibling `arcana-graph/src/snapshot/overlay_*.rs`, `overlay.rs` | shared overlay and graph tests |
| Initial import | `src/cli_commands.rs`, repository compiler, storage writer | CLI, repository, and storage tests |
| Changed-file update | `src/cli_update.rs`, `src/repository/ownership.rs`, `incremental.rs` | update, ownership, and incremental tests |
| Managed Lexicon synchronization | `src/cli_sync.rs`, `src/cli_sync_state.rs` | sync tests |
| Compaction | sibling `arcana-graph/src/snapshot/compaction.rs` | shared compaction tests |

Overlays may change edges only. Node-set changes require a packed rebuild.

## Related docs

- [Arcana architecture](ARCHITECTURE.md)
- [Application and operations](APPLICATION.md)
- [Lexicon ingestion contract](LEXICON_CONTRACT.md)
- [Current status](STATUS.md)

## Notes

Dense node identifiers are generation-local and may not be silently reinterpreted across a node-set change.
