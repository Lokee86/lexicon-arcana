# Lexicon Optimization Parity Audit

Parent index: [Development Documentation](INDEX.md)

**Audit date:** 2026-09-26

**Optimization oracle:** `c822f4d`

**Restoration project:** [Lexicon performance restoration](lexicon-performance-restoration.md)

## Purpose

Record the systematic comparison between the mature pre-port Go implementation and the Rust-owned Lexicon plus private Go semantic-helper path after Performance Restoration Phases 0–7.

The audit classifies implementation differences without treating the old implementation as mechanically authoritative. The semantic contract remains frozen: performance may change, but facts, identities, ownership, ordering, incremental semantics, and deterministic output may not.

## Overview

The audit covered discovery, parsing, identity construction, indexes, semantic targets, type relationships, calls, dataflow, SSA/VTA, dependency generation, fact materialization, incremental invalidation, deduplication, canonicalization, and serialization.

One additional lost optimization was found: the legacy object store wrote full-language file objects through a bounded worker pool, while the Rust materializer had become serial. Phase 8 restores bounded parallel full-file object writes with the same 16-worker ceiling and deterministic result ordering.

No other lost oracle optimization was found. Remaining differences are semantic requirements, intentional Rust architecture changes, obsolete adapter-boundary details, or measured follow-up candidates owned by later phases.

## Audit matrix

| Concern | Oracle behaviour | Current behaviour | Classification | Phase 8 disposition |
| --- | --- | --- | --- | --- |
| Discovery | Sorted repository walk over `.go` and `go.mod`, with the established excluded-directory set. | Rust owns the same semantic inventory and exclusions; content IDs are computed during discovery and ordinary Go source bytes are released. | Intentional Rust architecture change. | No action. Phase 3 reduced retained source bytes without changing discovery facts. |
| Parsing | Structural `go/parser` pass plus `packages.Load` syntax/type loading. | Private helper retains the same two semantic parsing roles. | Semantic requirement. | No action. |
| Identity construction | Go constructs semantic identities and hashes them immediately to Lexicon node IDs. | Helper emits canonical semantic identities; Rust validates them and constructs Lexicon IDs. | Intentional Rust architecture change. | No action. Repeated identity-to-ID work remains a Phase 9 measurement target. |
| Indexes | Hash maps back node existence, edges, semantic IDs, targets, and callsite lookups. | Helper uses typed hash indexes; Rust materialization uses `FactIndex` for node existence, ownership, and edge deduplication. | Restored parity. | Phase 1 removed materialization-as-database scans. |
| Semantic targets | Per-module `go/types` objects map to canonical callable/type targets. | Same per-module target construction and canonical namespace handling. | Semantic requirement / restored parity. | No action. |
| Type relationships | `go/types` method sets drive implements, extends, overrides, and interface implementation targets. | Same `go/types` relationships are derived from module-local typed state. | Semantic requirement / restored parity. | Phase 3 removed unused eager caches; Phase 5 restored typed-state lifetime. |
| Calls | Typed calls are resolved per file, merged by callsite, and reduced deterministically across shards. | Callsite accumulators preserve resolved-over-unresolved precedence, target union, call kind, and class priority during shard fan-in. | Restored parity. | Phase 4 restored early compaction. |
| Dataflow | Per-shard fact maps deduplicate semantic edges before repository merge. | Dataflow is keyed by complete semantic record identity and deduplicated within shards and fan-in. | Restored parity. | Phase 4 restored pre-IPC compaction. |
| SSA/VTA | Each module builds SSA, computes VTA callgraph outcomes, merges callsites, then releases compiler state. | Same module-local `AllPackages → Build → AllFunctions → VTA` lifecycle. | Semantic requirement / restored parity. | Phase 5 restored module-bounded lifetime. |
| Dependency generation | Go derives manifest dependencies and repository-local import dependencies. | Rust owns manifest parsing and local-import dependency materialization using helper declaration metadata. | Intentional Rust architecture change. | No action; measured cost is small relative to semantic analysis. |
| Fact materialization | Map-backed node/edge assembly avoids fact-vector scans. | `FactIndex` owns hot membership and owner lookup while the record vector remains output storage. | Restored parity. | Phase 1. Identity/hash/key micro-optimizations remain Phase 9 work. |
| Incremental invalidation | Stored dependency topology computes reverse impact and forward context; Go semantic units expand to package directories; unsafe changes fall back to full. | Rust planner/storage use the same closure rules and package-scoped temporary repository construction. | Restored parity. | Phase 6 restored package rather than module expansion. |
| Deduplication | Nodes, edges, semantic IDs, calls, and shard merges use keyed membership structures. | Hash-backed materialization plus keyed call/dataflow accumulators preserve deterministic final output. | Restored parity. | Phases 1 and 4. |
| Canonicalization | Facts are structurally ordered before output. | Rust uses structural primary and tie-break ordering; arbitrary JSON fields serialize only for otherwise-equal ties. | Restored parity. | Phase 2 removed serialize-every-fact sorting. |
| Serialization and storage | Adapter can stream JSON into partitioned analysis state; full-language file objects are written through a bounded worker pool capped at 16. | Native adapters return typed `Analysis`; private Go helper JSON is one typed decode; persistent objects use Rust binary encoding. Full-file object writes are now bounded and parallel again. | Streaming: obsolete oracle detail / intentional Rust architecture change. File writes: lost optimization. | Phase 7 removed duplicate helper decode. Phase 8 restores bounded full-file object writes. |

## Cross-cutting concurrency

The oracle used three material production concurrency boundaries: weighted language-plan execution, deterministic semantic shard workers/fan-in, and bounded full-language file-object writes.

The Rust path now has equivalents for all three. Phase 8 restores the missing storage worker pool using available parallelism, capped by file count and the oracle's 16-worker maximum. Logical shard sizing, worker budgeting, and fan-in remain separate from storage writes.

Concurrency *calibration* is not part of this parity audit. Phase 10 owns worker-count and shard-count tuning after the major algorithmic restoration is complete.

## Deferred measured candidates

The audit found several differences that may be optimizable but are not established regressions:

- memoizing semantic identity → Lexicon node ID conversion during one materialization pass;
- reducing full-string clones in hot edge/index keys;
- replacing ordered dependency-topology structures only if profiling shows they matter;
- revisiting streamed/framed helper responses only if protocol handling again becomes material;
- tuning worker/shard/fan-in settings against representative repositories.

Phase 9 subsequently retained scan-local semantic identity-to-node-ID memoization after a measured materialization improvement. It also tested and rejected compact interned edge keys because they regressed the measured materialization stage. The remaining items stay measurement-driven rather than being carried forward as presumed optimizations.

## Verification

The permanent parity gate remains semantic, not timing-based:

- native Go output must match the frozen legacy oracle fixtures;
- execution-shape changes must preserve deterministic output;
- incremental ownership and invalidation semantics must remain unchanged;
- storage object identity and manifest ordering must remain deterministic;
- the full Lexicon suite must remain green.

The bounded object-write restoration preserves input path order in returned `FileEntry` values even though physical writes execute concurrently.

## Related docs

- [Lexicon performance restoration](lexicon-performance-restoration.md)
- [Go adapter Phase 16 freeze](go-adapter-port-freeze-2026-09-26.md)
- [Testing and benchmarks](testing-and-benchmarks.md)
- [Lexicon Rust migration](../../lexicon/docs/RUST_MIGRATION.md)

## Notes

This audit is an optimization-parity record, not a claim that the oracle's implementation shape is preferable in every area. Rust ownership changes are retained where they preserve the contract and remove obsolete process/protocol boundaries. Future performance changes should be driven by measurement rather than by superficial source similarity.
