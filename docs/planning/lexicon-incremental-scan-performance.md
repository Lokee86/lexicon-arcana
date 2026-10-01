# Lexicon Incremental Scan Performance — Hard-Cut Repair

Parent index: [Planning](INDEX.md)
Status: Phases 0–4 implemented and validated on controlled fixtures. Phase 5 acceptance was attempted on 2026-10-01 and **DID NOT PASS**: the repaired full pinned-Hermes cold run timed out at 240 seconds without publishing; a pinned 14-file Hermes package comparison found repeated shared-fact full-analysis retries (12.98 and 13.30 seconds for one-file edits), a failed ten-file bound, and six changed-scenario exported-fact mismatches lacking an independent gold oracle. Package cancellation recovery and adapter-drift regression passed. No production cutover: profile native Python cold extraction, adjudicate shared-scope/fact differences, and rerun completed full pinned-Hermes acceptance. Evidence: [Phase 0](../development/lexicon-incremental-phase0-2026-10-01.md), [Phase 1](../development/lexicon-incremental-phase1-2026-10-01.md), [Phase 2](../development/lexicon-incremental-phase2-2026-10-01.md), [Phase 3](../development/lexicon-incremental-phase3-2026-10-01.md), [Phase 4](../development/lexicon-incremental-phase4-2026-10-01.md), [Phase 5 blocked acceptance](../development/lexicon-incremental-phase5-2026-10-01.md).
Owner: Lexicon repository mirror, Rust planner and storage. Arcana ingestion is unchanged.

## Purpose

Restore bounded, Git-like Lexicon warm-scan cost across source mirroring, scope planning and snapshot publication.

## Overview

The hard cut preserves existing incremental analysis semantics while replacing expensive repository-wide setup with a verified source delta and a snapshot-scoped persistent dependency index. Phases 0–5 separate evidence, mirror repair, index creation, incremental maintenance, deletion of the old full-object planner, and real-repository acceptance.

## Goal

Make the entire warm scan delta-proportional: detect source changes, determine safe dependency scope, analyze only that scope, merge changed objects, and publish an internally consistent snapshot. Keep semantic facts, object identities, conservative full-scan rules, and deterministic output unchanged. No compatibility planner, duplicate topology owner, or perpetual full-object dependency rebuild.

## Evidence and exact defect

- Prior Hermes diagnostic: 12,819 discovered files / 135 MB, 12,807 byte-equal comparisons, zero indexed skips, 12 copied files, about 22 seconds in source inventory; subsequent planning remained expensive. Reproduce rather than treating this run as a current benchmark.
- Current Rust implementation: repository/mirror_index.rs abandons all indexed skips when the private mirror is dirty. When enabled it hashes every desired source path, which also reads unchanged file content.
- storage/dependency.rs::dependency_data loads every stored object in a language, reconstructs node ownership and all cross-file edges, then computes the one-hop impact and context sets. The bounded planner added earlier is intact; obtaining its graph is not bounded.
- scan/planner.rs::plan_scan calls incremental_scope_with_additions for a partial plan; missing roots, unsupported additions, adapter/configuration drift, and structural changes must retain their conservative full-analysis semantics.
- A synchronous outer tool timeout can leave the underlying scan holding the repository lock. Diagnose lock/process lifecycle separately; do not disguise a slow scan by merely raising timeouts.

## Architectural contract

1. Published snapshot plus its immutable, verified dependency index is one logical generation. Never calculate scope using index data from a different generation.
2. The repository mirror owns source change detection; storage owns dependency topology; the scan planner consumes only a narrow scope query. No adapter or Arcana-owned replacement index.
3. Ordinary warm scans do not deserialize unrelated stored fact objects or read/hash unchanged source-file contents. Filesystem/Git metadata operations may still scale with the file inventory.
4. Indexed scope preserves current semantics: emit roots plus one-hop reverse dependents; context includes one-hop forward dependencies of the emitted set. Preserve deterministic normalization/order.
5. Python new-module unresolved-candidate checks, unknown/removed roots, shared-node ownership, renames, and manifest/configuration changes must be conservative; an unprovable scoped result requests full analysis.
6. Publication writes and verifies new immutable index data before making the corresponding snapshot CURRENT. Pending recovery and GC handle index references and consumer-pinned snapshots.
7. Legacy snapshots remain readable. A legacy generation may incur ONE verified topology bootstrap, persistently keyed to that exact snapshot/object set. It must not rebuild on every subsequent warm scan. If bootstrap cannot be validated, take the existing safe full-analysis path with an explicit reason.
8. A failed or interrupted scan must not corrupt CURRENT, its index, or the private source mirror's recoverable baseline.

## Chosen design and rejected alternatives

Chosen: snapshot-scoped, content-addressed topology with small independently replaceable index partitions; retain file-object IDs and per-file topology summaries, node-ID ownership lookup, reverse/forward file adjacency, and unresolved import-candidate lookup. Build it from analysis/materialization data on full publication. For incremental publication, replace affected file summaries, update directly affected ownership/reference partitions, and reuse unaffected partitions. If a changed/shared node can affect untouched references, use a reverse node-reference lookup to update their file relations, or deliberately force full analysis where precise repair is not provable. A large shared-object replacement must have measured, explicit cost.

Do NOT replace full fact-object decoding with decoding a monolithic whole-repository dependency blob on every scan. Do NOT append indefinitely growing delta overlays or store a second mutable authoritative graph. Sharding/layout are internal storage details behind the existing Store::incremental_scope_with_additions interface. Prefer compact indexed lookups and canonical deterministic encoding; avoid extra public API.

For source detection, maintain a baseline linked to the last published private-mirror generation. On Git-backed source trees, use baseline HEAD/tree differences plus staged, working-tree, previously dirty, newly untracked, deletion and rename candidates; verify candidate content against the last mirrored content ID. A file dirty in the private mirror must disqualify ONLY its own indexed shortcut, not every file. Source-Git HEAD changes, rebases and changes to ignore policy must trigger conservative reconciliation. Do not assume status relative to current source HEAD equals delta relative to Lexicon's previous dirty mirror.

For non-Git trees, unavailable baselines, racy timestamps, and ambiguous filesystem states, use a verified inventory/content comparison rather than trusting size and timestamp alone. Optimize the common case without introducing false unchanged decisions. Preserve excluded directories, symlink safety, case normalization and Windows path behavior.

## Implementation steps

### 0 — Instrument and pin an honest production baseline

- Add opt-in timers/counters for source path discovery, candidate derivation, source bytes read/hashed, indexed skips, byte-equality fallbacks, changed paths, dependency index loads/partitions, fact-object reads, selected emit/context, adapter analysis, merge, publication, total wall time, peak RSS.
- Capture a clean scan, one-file modification, second modification while the private mirror is dirty, new/delete/rename, and interrupted scan on a disposable Hermes checkout; record exact source revision, Lexicon binary revision and snapshot ID.
- Run a separate full Python adapter benchmark for comparison. Do not conflate adapter-only analysis with full cold source-to-snapshot publication.
Gate: repeatable stage-level evidence and current correctness baselines; no scan algorithm changed.

### 1 — Repair mirror change detection without changing semantics

- Introduce a generation-bound mirror baseline and per-path candidate set; eliminate the mirror_clean all-or-nothing fast-path condition.
- Reuse proven unchanged paths; compare/hash only change candidates or ambiguous paths. Track prior dirty files so reverting an edit is detected.
- Reconcile changed ignore rules, untracked discovery, staged content, deleted files, rename/case changes, source HEAD changes and clean/dirty transitions.
- Keep an explicit safe full-inventory fallback for non-Git or unverifiable state. Emit the fallback reason instead of silently reverting to thousands of byte comparisons.
Gate: second one-file Hermes scan reads no unrelated source bodies; fixture tests prove exact source mirror bytes and delta parity against the exhaustive legacy path, including Windows cases.

### 2 — Introduce a snapshot-bound persistent dependency index

- Extract an internal IndexBuilder/IndexReader boundary from dependency_data. The reader returns exactly the existing IncrementalScope contract.
- Build the initial index from records already available during full materialization, including shared nodes, unresolved candidates, file ownership and node references. Do not reread every stored object just to build a fresh index.
- Persist versioned immutable, content-addressed partitions and an index root referenced by LanguageEntry with a defaultable field; verify schema/version and referenced object/generation identities.
- Support one-time, persisted legacy bootstrap keyed by exact snapshot identity. Distinguish missing legacy index, corrupt current index, and safe forced full analysis.
Gate: fixture oracle compares new and old scope results byte-for-byte; one-file scope queries load no unrelated fact objects and no whole-language topology blob.

### 3 — Maintain the index incrementally during publication

- Extend full and incremental materialization to produce/reuse per-file topology summaries and changed node-ownership/reference partitions.
- Recompute only adjacency affected by changed/removed files or changed node ownership. Handle shared-record replacement explicitly; an unsafe topology transition must force full analysis, never produce a silently partial index.
- Thread the new index root through pending publication, recovery, current snapshot validation and snapshot/consumer-pin GC. No orphaned CURRENT and no index generation mismatch after interruption.
Gate: full-vs-incremental fact identity and dependency-scope parity across two sequential edits, undo, rename/delete, Python additions, changed ownership, failed writes, crashes before/after CURRENT, recovery and GC.

### 4 — Remove the repository-wide incremental planning path

- Hard-cut dependency_data's per-scan all-object loader from the production planner; preserve it only as a non-production oracle if useful for parity tests, then delete redundant code.
- Keep current scope, context, deterministic ordering, unsupported-language addition and fallback semantics. Review whether shared merge independently rereads large object sets and measure it; do not expand this repair into unrelated adapter changes.
- Add a counter assertion to the production entrypoint: one-file warm scan must not load unrelated fact objects or byte-compare unrelated source files.
Gate: all existing planner, snapshot/storage, Python and enabled-language suites green; synthetic fixture scaling demonstrates that one-file workload counts stay bounded as unrelated file count grows.

### 5 — Production acceptance and operational cutover

- Measure no-change, one-file edit, repeated dirty-mirror edit, 10-file edit, add/delete/rename, adapter drift and full cold scan on pinned Hermes checkout. Use the same hardware, binary, cache state and output-verification method across before/after runs.
- Provisional release targets for a warmed indexed Hermes checkout: no-change scan <=5 seconds; one-file edit scan <=10 seconds AND <=20% of measured full-adapter time; no unrelated fact-object reads and no unrelated source-content reads. Establish a defensible bound for 10-file edits from the baseline rather than inventing it.
- Require semantic-fact and snapshot parity, safe fallback coverage, bounded peak-RSS measurements, and clean lock release on success, failure and cancellation.
- Update Lexicon current architecture/behavior docs, behavioral contract matrix, performance-restoration evidence, planning status and installed consumer guidance. Arcana remains independently queryable from its already published snapshot while a deliberate Lexicon refresh runs.
Gate: actual lexicon scan production path passes, not just planner microbenchmarks; record dated benchmark artifacts and any unmet performance target explicitly.

## Execution order and scope control

Execute 0, 1, 2, 3, 4, 5 in order. Each phase gets its own tests and commit. Use the isolated perf/lexicon-incremental-scan worktree; do not modify the active C-family branch. A later stage may be split if index storage/recovery grows beyond a reviewable change. Do not start by rescanning Hermes repeatedly or refreshing Arcana to generate the design.

Non-goals: Arcana graph storage refactor, new language parsing behavior, altered dependency depth, broad adapter rewrites, increasing scan timeouts as a performance fix, or retaining the old full-object scope builder as a compatibility fallback in production.

## Related docs

- [Phase 0 baseline and instrumentation evidence](../development/lexicon-incremental-phase0-2026-10-01.md)
- [Lexicon dependency semantics](../../lexicon/docs/DEPENDENCY_SEMANTICS.md)
- [Testing and benchmarks](../development/testing-and-benchmarks.md)

## Notes

The pinned-Hermes cold run was deliberately terminated at its diagnostic limit and is not a completed benchmark. The full production before/after gate remains a Phase 5 requirement.