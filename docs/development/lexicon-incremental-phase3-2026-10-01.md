# Lexicon incremental scan Phase 3 — delta-maintained dependency index

Parent index: [Development Documentation](INDEX.md)

## Purpose

Maintain exact snapshot-bound dependency-index generations across ordinary incremental publications. Reuse untouched immutable partitions so repeated edits never require the Phase 2 whole-language bootstrap.

## Overview

Full analysis already materializes a partitioned dependency index. Phase 3 adds deterministic, copy-on-write updates to the same index owner and embeds a new generation-bound root in every successful scoped incremental language entry. The reader and planner API do not change. Index root/partition writes happen before the candidate manifest reaches PENDING and CURRENT.

## Ownership and delta algorithm

- `dependency_index_partition.rs` implements the internal typed CAS partition editor: load only the requested 1-of-64 shard, compare values, rewrite dirty shards, and reuse unchanged IDs.
- `dependency_index_parts.rs` has the single file-owned fact-to-topology extraction rule shared by full and incremental indexing. Its shared-node path lookup records node IDs even for paths not yet present.
- `dependency_index_delta.rs` replaces only edited/removed file summaries, ownership entries for affected node IDs, touched unresolved-import candidate sets, and target-reference lists. It uses target→source references to find untouched neighbours requiring recomputation when a node's owner changes.
- `dependency_index_delta_links.rs` disconnects old cross-file reverse edges, updates changed file evidence, recomputes changed and referencing sources' outgoing edges against new ownership, then reconnects reverse edges. Only reached partitions are loaded or rewritten.
- Version 2 index roots add immutable shared-node path partitions. Existing version 1 roots remain readable; the first incremental update from an old index reads its one previous shared fact object if present to upgrade this lookup, then publishes a version 2 root. The content-addressing hash domain stays unchanged.
- An incomplete/unprovable ownership transition returns `UnsafeIndexDelta`. The scan coordinator retries complete analysis instead of publishing potentially stale topology. Complete shared-object updates that merge to exactly the old object ID stay scoped; actual shared-object changes use full analysis.

## Publication, recovery and retention

A candidate index root's signature covers the exact resulting language metadata, file-object IDs and shared-object ID. `Store::publish`, used for ordinary publication and PENDING recovery, verifies the root and all reachable shards before CURRENT advances. Interrupted publication leaves the previous generation intact. A corrupt pending root prevents recovery without silently replacing CURRENT.

GC tracks current, retained and consumer-pinned snapshots' topology roots and all referenced immutable shards, including exact-snapshot legacy-bootstrap sidecars. It collects orphaned topology objects and bootstrap directories of expired snapshots. GC refuses to operate while PENDING exists. Execution rechecks consumer pins and reachable objects to reject plans made stale by new pins or bootstrap creation.

## Verification

- Incremental/full rebuild parity: consecutive edits, relationship changes, deletion, adding a path that activates a previously shared node, and a subsequent edit. A separate compatibility test starts from a genuine historical version-1 root without shared-path partitions, upgrades on an addition, and confirms the resulting root ID equals a fresh version-2 full rebuild. The test checks exact content-addressed index-root IDs, fact-object IDs, source scopes, one-hop relations and Python addition decisions after each generation.
- Crash boundary: corrupt a pending index root, verify recovery fails without moving CURRENT and GC is blocked, restore bytes, recover successfully, then query the new index with fact objects unavailable.
- Lifecycle: pin an old indexed snapshot while newer generations and orphan roots exist; collect unreachable data without deleting pinned roots, remove the pin and collect the retired root, then ensure a legacy bootstrap directory is removed when its snapshot expires.
- Production CLI fixture: a disposable Git-backed 1,001-file Python fixture records no-change, successive one-file edits and private-mirror repair. The acceptance gate requires both edited generations to show `scan.dependency_index_query` with zero fact-object reads, and **no** `scan.dependency_bootstrap` or `scan.dependency_rebuild`.

### Measured production CLI baseline

The instrumented development Rust CLI completed five scenarios on the disposable Git-backed 1,001-file Python source fixture. Raw stage evidence and exact snapshot IDs: `lexicon/evaluation/performance/incremental-phase3-git1000-2026-10-01.json`.

| Scenario | CLI wall | Source index skips | Dependency query | Index delta | Whole-language bootstrap |
| --- | ---: | ---: | ---: | ---: | --- |
| Initial full publication | 10.375 s | 0 (expected first copy) | not applicable | full index construction | none |
| Unchanged | 1.407 s | 1,001 | none needed | none | none |
| First one-file edit | 2.593 s | 1,000 | 0.630 ms; 1 shard; 0 object reads | 32.378 ms; 8 loaded shards; 0 rewritten | **none** |
| Second one-file edit | 2.704 s | 1,000 | 0.894 ms; 1 shard; 0 object reads | 37.075 ms; 8 loaded shards; 0 rewritten | **none** |
| Dirty-mirror repair | 1.656 s | 1,000 | none needed | none | none |

The two edits were comment-only: file content IDs and snapshot index-root signatures changed, but the semantic adjacency and ownership partitions were identical. Reusing every unchanged partition is therefore correct (`rewritten_partitions=0`). The separate relationship/ownership parity suite covers cases that genuinely rewrite shards.

For comparison, Phase 2's second edit bootstrapped by reading **1,002** stored fact objects and spent **481.591 ms** constructing its one-time replacement. These are separate disposable, development-profile runs; total wall-time differences are not controlled percentage speedups.

The controlled fixture confirms the delta algorithm and publication path but does not substitute for the pinned-Hermes full before/after acceptance in Phase 5.

## Limitations

This phase retains the legacy one-time whole-language bootstrap for older unindexed snapshots. Its removal from normal planning and boundary clean-up belongs to Phase 4. A shared-object replacement that cannot be proved identical and an ownership collision take a safe full-analysis route rather than attempting a potentially global incremental repair. Filesystem and Git metadata discovery still has inventory-sized work even though unchanged file bodies and dependency objects no longer need to be read.

## Code map

| Boundary | Ownership |
| --- | --- |
| Partition CAS, shared path encoding | `lexicon/src/storage/dependency_index_partition.rs`, `dependency_index_parts.rs`, `dependency_index_model.rs` |
| Delta and link maintenance | `dependency_index_delta.rs`, `dependency_index_delta_links.rs`, `materialize.rs` |
| Conservative full retry | `lexicon/src/scan/execute.rs` |
| Retention and crash safety | `lexicon/src/storage/gc*.rs`, `pending.rs`, `store.rs` |
| Regression gates | `lexicon/tests/dependency_delta.rs`, `dependency_recovery.rs`, `storage_gc.rs`, `storage_topology_gc.rs`, `dependency_index.rs`, `dependency_upgrade.rs`, `scan_engine.rs` |
| Production fixture | `lexicon/evaluation/performance/incremental-phase3-git1000-2026-10-01.json` |

## Related docs

- [Repair plan](../planning/lexicon-incremental-scan-performance.md)
- [Phase 2 index baseline](lexicon-incremental-phase2-2026-10-01.md)
- [Snapshot contract](../../lexicon/spec/snapshots-v1.md)
- [Testing and benchmarks](testing-and-benchmarks.md)

## Notes

A `scan.dependency_index_delta` event counts touched files, affected referencing files, loaded/rewritten partitions and only the fact-object reads directly attributable to a legacy version-1 shared-path upgrade. Merge-time shared-object reads remain separately accounted by the materialization stage. Do not conflate total CLI time with the scoped index query.
