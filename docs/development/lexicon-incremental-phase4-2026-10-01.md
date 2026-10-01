# Lexicon incremental scan Phase 4 — Remove per-scan dependency reconstruction

Parent index: [Development Documentation](INDEX.md)

## Purpose

Remove the old whole-language fact-object loader from production dependency planning. Preserve exact legacy snapshot semantics with an isolated, once-per-snapshot migration and assert bounded work through the real CLI.

## Overview

The storage planner now has one dependency lookup seam: `Store::index_for_snapshot(snapshot_id, language_entry)`. For an indexed generation it returns its immutable index ID without decoding fact objects. A legacy generation without an index uses a verified, persisted, exact-snapshot bootstrap once. Both cases then run the same partitioned `IndexReader` and retain existing scope semantics.

The legacy path no longer builds a parallel adjacency graph or maintains a separate ownership interpretation. `dependency_index_legacy.rs` decodes historical file/shared fact objects once, borrows their records, and calls the same `index_from_records` as full materialization. `dependency_data` and its redundant file-object collector are deleted. The incremental publisher resolves historical indexes through the same storage seam, not through a dummy planner query.

## Contract and failure behaviour

- Indexed scope: the requested roots plus one hop of reverse dependents are emitted; context adds one forward hop. Roots absent from the snapshot and unsupported or sensitive module additions retain conservative full fallback. Paths are normalized and results sorted.
- Indexed generations never deserialize repository-wide fact objects for dependency lookup, and no second mutable dependency graph exists. Source-Git verified skip eligibility remains a separate Phase 1 concern.
- Historical snapshots preserve their original canonical bytes. First use can decode the old file/shared objects once to materialize immutable topology and write a pointer bound to the exact snapshot and entry signature. Later queries and an ensuing first incremental index update reuse that pointer without rereading historical file objects.
- An invalid index ID, corrupt partition, mismatched signature or failed one-time bootstrap returns an error to the planner. Its existing error path selects complete-language analysis rather than trusting unverifiable partial topology.
- The separate topology safety check can decode *selected prior file objects*, one per selected existing path, to decide whether a newly produced relationship requires full analysis. These related, bounded reads are not dependency graph reconstruction.
- If an adapter requests complete shared replacement, shared-object merge may decode the previous shared object and prior **touched** file objects. `scan.shared_merge` now reports previous shared-record count, touched-file object reads and total fact-object reads. A genuinely changed shared object retains Phase 3's conservative complete-language retry; unrelated adapter or shared-storage redesign is excluded.

## Production-path guardrail

`incremental_phase0.py --git-source --assert-bounded --max-steps 4` executes the real Rust CLI on a disposable Git-backed checkout, and checks both successive one-file edits *at the entrypoint*. The reusable `incremental_phase4_gate.py` verifies:

- An unchanged scan preserves its snapshot and indexes all source paths.
- Each edit copies one source and skips every unrelated source without byte-equality fallback.
- Indexed dependency lookup reads zero stored fact objects and loads at most four partitions; index update reads zero extra fact objects and at most sixteen partitions for this controlled leaf-edit fixture.
- The topology safety check reads no more than the number of selected files, and the selected set cannot exceed the indexed emission set.
- No ordinary edit triggers a repository-wide bootstrap/rebuild or an unintended full-analysis retry.
- Comparing fixture sizes also limits partition-load count growth, rather than claiming total filesystem/Git metadata time is independent of repository size.

`test_incremental_phase4_gate.py` unit-tests positive cases and intentionally failing metrics. Synthetic fixtures vary the amount of unrelated code. The pinned-Hermes before/after time and peak-RSS acceptance remains Phase 5.

## Verification

Direct parity and lifecycle tests cover full-versus-incremental index identities, additions, deletions, shared-node ownership changes, version-1 root upgrade, legacy-snapshot bootstrap reuse, a migrated snapshot's first incremental update *with old stored fact objects unavailable*, corrupted partitions, recovery and GC. Broader suites include the Python adapter, enabled-language planning, scope execution, publication and snapshot compatibility. A direct shared-merge fixture exercises complete-but-identical shared replacement. With `LEXICON_PERF=1`, it reported `previous_shared_records=1`, `changed_file_objects_decoded=1` and `fact_object_reads=2` (one prior shared object and exactly one touched prior file), plus an index delta with zero fact-object reads.

## Controlled production CLI evidence

The development Rust CLI ran two disposable Git-backed Python fixtures using `--assert-bounded --max-steps 4`. These runs used the Phase 4 worktree binary built immediately before this phase's commit; their JSON `lexicon_revision` field records the prior HEAD `f3f1889`, not a separate full-repo baseline. The source trees are synthetic and their wall times are not controlled paired comparisons.

| Metric | 61 source files | 1,001 source files |
| --- | ---: | ---: |
| Initial full CLI scan | 3.062 s | 16.063 s |
| Unchanged CLI scan | 1.594 s | 2.234 s |
| First one-file edit | 3.000 s | 3.219 s |
| Second one-file edit | 2.297 s | 3.968 s |
| Indexed source skips on each edit | 60 | 1,000 |
| Unrelated source-byte comparisons | 0 | 0 |
| Dependency lookup partitions per edit | 1 | 1 |
| Delta index partitions loaded per edit | 8 | 8 |
| Dependency index fact-object reads per edit | 0 | 0 |
| Prior fact-object reads for topology safety per edit | 1 (selected file) | 1 (selected file) |
| Whole-language dependency bootstrap on either edit | none | none |

The production gate passes separately at each size and jointly rejects growth in unrelated dependency-partition load count. Both edits are comment-only: semantic adjacency is unchanged, so all eight loaded maintenance partitions are correctly *reused*, with zero partition rewrites. Separate full-index parity tests exercise changed relationships, shared-node ownership and unresolved import candidates, where rewritten partitions are required.

Artifacts: `lexicon/evaluation/performance/incremental-phase4-git60-2026-10-01.json` and `incremental-phase4-git1000-2026-10-01.json`. The parent directory's `incremental_phase4_gate.py` verifies the raw counts and has a six-test unit suite. Full Hermes latency/RSS and warm/cold comparison remain Phase 5.

## Code map

| Responsibility | Implementation |
| --- | --- |
| Narrow planner and stable public methods | `lexicon/src/storage/dependency.rs` |
| One index-resolution seam | `lexicon/src/storage/dependency_index_resolve.rs` |
| Isolated legacy one-time migration | `lexicon/src/storage/dependency_index_legacy.rs` |
| Common deterministic index builder | `lexicon/src/storage/dependency_index_build.rs` |
| First migrated incremental generation | `dependency_index_delta.rs` |
| Scoped topology-safety decode counter | `lexicon/src/storage/topology.rs` |
| Shared merge read counter | `lexicon/src/storage/materialize_merge.rs` |
| Production CLI gate and unit tests | `lexicon/evaluation/performance/incremental_phase4_gate.py`, `test_incremental_phase4_gate.py` |
| Legacy migration parity | `lexicon/tests/dependency_legacy_cutover.rs` |

## Related docs

- [Incremental scan repair plan](../planning/lexicon-incremental-scan-performance.md)
- [Phase 3 delta-maintained index](lexicon-incremental-phase3-2026-10-01.md)
- [Testing and benchmarks](testing-and-benchmarks.md)
- [Lexicon snapshot contract](../../lexicon/spec/snapshots-v1.md)

## Notes

A topology safety check decoding one changed prior file is legitimate selected-object work, not an unrelated dependency fact-object read. Report stage counters separately to avoid representing this as zero total fact-object reads. Debug/development CLI elapsed time and versioned benchmark snapshots are measured evidence, not an unverified production-Hermes speedup claim.
