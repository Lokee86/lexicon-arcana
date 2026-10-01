# Lexicon Incremental Scan Phase 2 — Persistent Dependency Index

Parent index: [Development Documentation](INDEX.md)

## Purpose

Make ordinary warm dependency scope queries load only the relevant immutable index partitions, not every stored fact object, while preserving the existing one-hop dependency semantics and safe full-analysis fallback.

## Overview

Full language materialization builds a deterministic dependency index directly from borrowed records already grouped in memory. The immutable index root is referenced by the language manifest entry and bound to the complete entry's file/shared object identities and analysis metadata. The reader loads only file adjacency partitions for requested roots, reverse dependents, their forward context and relevant unresolved Python module candidates. Unknown roots, ambiguous additions, missing or corrupted index objects fail closed.

Legacy manifests, including interim Phase 2 incremental results, lack the index root. The first scope query bootstraps an index **once for that exact snapshot ID** from its old objects, then writes an atomic sidecar pointer. Later queries use the verified index without rereading those objects. Phase 3 will maintain changed index partitions during incremental publication and stop requiring that bootstrap after each edit.

## Format and ownership

- `LanguageEntry.dependency_index_id` is an optional, empty-by-default manifest field. Legacy JSON remains readable and unchanged when the field is empty.
- Root and partitions are immutable, domain-separated, content-addressed JSON under `topology/objects/`, isolated from ordinary fact objects. The root records schema version 1, a verified language-entry signature, and partition object IDs for file topology, node ownership, target references and unresolved candidate lookups. The root contains **partition pointers**, not a whole-repository adjacency graph.
- Files are hashed into 64 deterministic partitions. A file record holds one-hop forward/reverse file relations plus the local node, referenced-target and unresolved-candidate evidence needed by Phase 3. Global node-owner and target-reference partitions are already persisted for the following maintenance phase.
- Root entry signatures cover language metadata, file object identities and shared object ID, with the optional root field cleared when computing the signature. Reusing an index with changed facts or config therefore fails verification.
- `Store::publish`, including pending-publication recovery, verifies the root and every referenced immutable partition **before** updating CURRENT.
- Legacy bootstrap sidecars bind both the exact snapshot ID and entry signature; invalid or stale pointers return an error rather than silently reconstructing or reusing mismatched topology.

## Semantics and fail-closed rules

- Emit = requested roots plus their direct reverse dependents; context = emitted files plus their direct forward dependencies. Inputs/outputs remain deterministic and sorted.
- Roots absent from the stored language require full analysis. Added Python modules require full analysis if they can satisfy a previous sensitive unresolved candidate; non-Python additions retain the existing conservative full-analysis rule.
- File-owned edges resolve target node IDs through the global node-owner mapping. Shared nodes override ownership only where their normalized path is a known stored file, matching the previous decoder.
- Missing or corrupt root/required partition errors propagate to the planner, which already selects its safe full-analysis path.
- Index creation from full-analysis records performs **zero fact-object reads**; index scope queries perform zero fact-object reads and load only visited partitions. Full legacy bootstrap is explicitly the sole temporary whole-language decoder.

## Controlled production CLI measurements

The production Rust CLI was built from the Phase 2 worktree and run through the disposable Git-backed 1,001-file Python fixture. The harness recorded its prior HEAD as `dc60d18`; the implementation had uncommitted Phase 2 changes, subsequently committed together with this report. The fixture and source revision are recorded in `lexicon/evaluation/performance/incremental-phase2-git1000-2026-10-01.json`. These are development-profile samples, not a controlled full-Hermes baseline.

| Operation | CLI wall | Index scope | Fact objects decoded during scope | Notes |
| --- | ---: | ---: | ---: | --- |
| Initial full build | 9.485 s | built 256 immutable partitions in 1,071 ms | **0 extra reads** | First index built from in-memory analysis |
| Unchanged scan | 1.265 s | no scope necessary | 0 | 1,001 source files indexed/skipped |
| First one-file edit | 2.079 s | **0.650 ms**, 1 partition | **0** | Incremental Python adapter 3.063 ms |
| Second one-file edit | 2.484 s | 0.530 ms, 1 partition | **1,002** once for bootstrap | Missing post-merge index: 481.591 ms bootstrap |

The second edit's bootstrap is deliberately marked as the **Phase 3 gap**: the previous incremental snapshot has no maintained index root, so the next snapshot needs its own one-time bootstrap. The first edit demonstrates Phase 2's actual reduction: no unrelated fact objects were loaded for scope planning. Neither timing proves the final Hermes release targets.

## Verification and release boundary

Targeted regressions: full-vs-legacy scope parity (including shared-node ownership, one-hop depth, missing roots, Python additions), object-directory removal proving warm queries are independent of fact objects, exact-snapshot bootstrap reuse, generation mismatch rejection and corrupted index rejection.

This is NOT the full delta-proportional scan repair yet. Until Phase 3, `build_incremental_language` deliberately publishes a language entry **without** the old root: the next generation requires a one-time bootstrap, rather than risking stale adjacency. Phase 3 owns precise changed-partition publication, GC/consumer-pin retention and recovery lifecycle. Phase 4 removes any remaining production dependency on legacy whole-object scope reconstruction; Phase 5 owns pinned-Hermes end-to-end before/after acceptance.

## Code map

| Boundary | Files |
| --- | --- |
| Content-addressed schema and generation binding | `lexicon/src/storage/dependency_index_model.rs`, `dependency_index_store.rs` |
| Full build from existing owned/shared records and legacy bootstrap writer | `dependency_index_build.rs` |
| Narrow visited-partition query | `dependency_index_read.rs` |
| Manifest lifecycle | `model.rs`, `materialize.rs`, `store.rs` |
| Legacy once-only bootstrap | `dependency.rs` |
| End-to-end parity, corruption and no-fact-read tests | `lexicon/tests/dependency_index.rs` |

## Related docs

- [Overall incremental performance plan](../planning/lexicon-incremental-scan-performance.md)
- [Phase 0 baseline](lexicon-incremental-phase0-2026-10-01.md)
- [Phase 1 Git mirror](lexicon-incremental-phase1-2026-10-01.md)
- [Testing and benchmarks](testing-and-benchmarks.md)

## Notes

Performance gate requires actual CLI evidence on a disposable fixture with a fully published index. Full Hermes scans are deferred until Phase 5 to avoid repeated long initialization and unnecessary workload during this storage repair.
