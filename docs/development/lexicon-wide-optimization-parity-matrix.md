# Lexicon-Wide Pre-Port Optimization Parity Matrix

Parent index: [Development Documentation](INDEX.md)

**Audit date:** 2026-09-27
**Rust baseline:** `b75daae` — Phase 4 materialization copy removal
**Scope:** mature Lexicon performance work immediately preceding the Rust migration

## Purpose

Record every performance optimization in the mature pre-port Lexicon sequence and its current Rust disposition so future ports/refactors cannot silently discard established performance architecture.

This matrix is separate from the [Go-path optimization parity audit](lexicon-optimization-parity-audit.md). That audit covers the later Go adapter/helper restoration. This matrix covers the Lexicon-wide core, Python, storage, C#, and TypeScript optimization work that existed before the Rust-first migration began.

Semantic facts, identities, ownership, ordering, incremental behavior, snapshot determinism, and adapter correctness remain hard invariants. An optimization is not considered restored if it changes those contracts.

## Overview

The matrix treats mature pre-port behavior as a performance oracle rather than an implementation template. Each optimization is classified as restored, structurally superseded, retained as a migration oracle, or assigned to an explicit later phase so performance properties cannot disappear silently during migration.

## Audited pre-port commits

The direct mature performance sequence before the Rust migration is:

| Commit | Change |
| --- | --- |
| `6d31fad` | Generalize partitioned scans and parallelize Python analysis |
| `5001baa` | Reduce Lexicon rebuild and publication overhead |
| `a4a88ba` | Stream Python analysis into Lexicon |
| `a7f8762` | Compact Lexicon durable fact objects |
| `fd72318` | Reduce Lexicon Python analysis memory |
| `2c96c1a` | Optimize C# fact dedup and audit adapter scans |
| `e6d51ab` | Optimize TypeScript dispatch and output emission |

`81b0ed8` is a planning-only commit and is represented by the implementation commits it roadmapped. Later semantic-coverage commits are excluded because this is a performance-parity audit, not a semantic-port inventory.

## Status definitions

- **Restored:** the Rust/current path has the same performance property and semantic contract.
- **Retained migration oracle:** the optimized external adapter source remains preserved for its pending/native migration even when the current Rust host does not register that adapter.
- **Superseded:** the old cost or boundary no longer exists in the Rust architecture.
- **Phase 5 restore:** the audit found a real lost optimization and restored it here.
- **Pending Phase 3:** the optimization remains intentionally outstanding and has an explicit owner.

## Permanent parity matrix

| Oracle optimization | Pre-port behavior | Current disposition | Evidence / owner |
| --- | --- | --- | --- |
| Weighted Python logical sharding | File work was partitioned by source-size weight rather than raw file count. | **Restored.** | `python/extract/parallel.rs::partition_ranges`; Phase 1. |
| Bounded Python extraction workers | File-local work used a bounded worker pool. | **Restored.** | `extract_repository` caps active workers to planned ranges; Phase 1 execution-shape equality test. |
| Deterministic shard merge order | Worker completion order could not change fact collision/order behavior. | **Restored.** | Rust fragments are sorted by shard index before fan-in reduction; Phase 1. |
| Configurable merge fan-in | Shard reduction used the scan plan's fan-in rather than one monolithic merge. | **Restored.** | `reduce_fragments(..., merge_fan_in)`; Phase 1. |
| File-local semantic work inside extraction workers | Semantic AST walks ran before the full file AST left the worker lifetime. | **Restored.** | `semantic::emit_file_facts` runs inside `extract_range`; Phase 1. |
| Small Windows IPC units | Python process workers returned file-local compact results instead of huge AST-bearing shards. | **Superseded.** | Native Rust uses scoped in-process threads; there is no Python pickle/IPC boundary. |
| Adapter fingerprint ignores tests/build noise | Test trees/files did not invalidate production adapter fingerprints. | **Superseded.** | Native adapters hash explicit compile-time production source lists with `source_fingerprint`; test files are not enumerated. |
| Node-only Interstack object loading | Interstack verified object identity but decoded only node sections, not edges/unresolved. | **Phase 5 restore.** | `interstack/build.rs` now uses `Store::load_node_facts`; node-only decoder regression proves relationship payloads are skipped. |
| Bounded parallel full-file object writes | File CAS objects were written concurrently with a 16-worker ceiling and deterministic returned order. | **Restored.** | `storage/materialize_parallel.rs`; existing bounded-worker test; retained through Phase 4 borrowed materialization. |
| Stream Python adapter output instead of building one JSONL blob | Python records streamed through the process/transport boundary. | **Superseded.** | Python is now a native typed Rust adapter returning `Analysis`; no subprocess JSONL transport exists. |
| Partition streamed analysis by owner during decode | The object store built owner/shared partitions as records arrived instead of retaining one complete flat record stream. | **Pending Phase 3.** | Phase 4 removed duplicate materialization copies, but authoritative `Analysis` is still a flat `Vec<FactRecord>`. Phase 3 owns the partitioned analysis core. |
| Binary v2 durable fact objects | Compact structural encoding replaced verbose durable representation without pruning facts. | **Restored.** | Current `storage/binary/v2_*` reader/writer; v1 and legacy read compatibility retained. |
| Compact SHA-256 identities | Digest identities used binary representation where the format permits. | **Restored.** | Binary v2 identity encoding. |
| Local node ordinals + bounded external references | Repeated node IDs were not serialized as full strings inside each relationship. | **Restored.** | Binary v2 table/reference encoding. |
| Stable kind/relation codes with fallback | Common enums use compact codes without losing unknown values. | **Restored.** | Binary v2 common/table encoding. |
| Repeated owner/path/qname factoring | Object-level identity removes repeated record strings. | **Restored.** | Binary v2 factored fields and qname encoding. |
| Front-coded deterministic string table | Sorted string-table prefixes reduce durable duplication. | **Restored.** | Binary v2 writer/reader and existing codec/golden tests. |
| Bound/release Python source-byte caches | Duplicate encoded source representations were released after file-local extraction. | **Restored.** | Source is loaded per file inside the worker lifetime; Phase 1 retained-source metrics. |
| Remove per-file line-string copies | Source was no longer duplicated into a line-vector representation. | **Restored.** | Rust `SourceFile` has bytes/source/AST but no retained line-vector field. |
| Release source bytes/text/full AST roots before global resolution | Repository-wide resolution retained only the fragments it actually needed. | **Restored.** | Loaded `SourceFile` dies at the end of each `extract_range` iteration; Phase 1 peak AST/source counters. |
| Release merged shard containers | Source fragments were emptied/dropped while reducing shards rather than retained alongside the merged state. | **Restored.** | Rust `Facts::merge_from` consumes source fragments by value; reduction drops merged fragments immediately. |
| Retain only resolver AST fragments | Function/class/call/binding structures kept only expressions/arguments needed later, not file roots. | **Restored.** | Current Python model retains resolver-specific AST fragments only; no repository-wide file AST collection. |
| Remove duplicate repository-wide symbol-kind index | A redundant global index was deleted. | **Restored.** | Current Python `Facts` has no `symbol_kinds` index. |
| Keep dataflow dedup file-local | A repository-wide dataflow dedup set was removed after durable edges were emitted. | **Restored.** | Current Python `Facts` has no `dataflow_edges` repository-wide set. |
| Compact durable Python fact records | Python dictionaries were replaced with compact slotted records and materialized only at emission. | **Superseded.** | Rust facts are typed structs rather than Python dictionaries; no dict materialization layer exists. |
| Release resolver-only Python state before canonical materialization | Imports, calls, functions, classes, bindings, and indexes were cleared after final facts existed. | **Restored.** | `Facts::release_analysis_state`; Phase 2. |
| Structural C# edge/unresolved dedup keys | C# stopped canonical-JSON serializing every record merely to deduplicate it. | **Retained migration oracle.** | `adapters/csharp/Facts.cs::EdgeKey` / `UnresolvedKey` preserve the optimization; C# is defined but not yet registered by the current Rust `AdapterHost`. |
| Indexed TypeScript method dispatch | Method-name dispatch candidates were indexed once rather than rescanning all declarations for every call. | **Retained migration oracle.** | `adapters/typescript/src/call-targets.ts::buildDispatchIndex` preserves the optimization; TypeScript is not yet registered by the current Rust `AdapterHost`. |
| Stream TypeScript JSONL writes | Output was written record-by-record instead of allocating one repository-sized joined string. | **Retained migration oracle.** | `adapters/typescript/src/emission.ts::writeJsonl` still writes record-by-record; the adapter is not yet on the native Rust host path. |

## Phase 5 finding

The audit found exactly one unintentional parity loss on the audited pre-port sequence: the Rust Interstack port regressed from node-only fact-object reads to full object decoding. Phase 5 restores the node-only path.

The owner-partitioned analysis core is not counted as an accidental Phase 5 miss. It was already identified as **Phase 3** and remains intentionally pending. Phase 4 removed the worst copy amplification without changing that ownership boundary.

No other audited optimization is unowned. Each row is now either restored, retained, structurally superseded, or explicitly assigned to Phase 3.

## Permanent gate

Future performance/refactor work should update this matrix whenever it changes one of the audited boundaries. A change is acceptable only if it either:

1. preserves the listed performance property and semantic contract;
2. supersedes the old cost by removing the boundary entirely; or
3. records a named replacement owner and validation gate before removing the optimization.

The normal protecting gates are:

- frozen Python canonical fixture hashes;
- execution-shape determinism;
- binary v2 round-trip/golden/object-identity tests;
- node-only decode regression;
- incremental ownership/invalidation tests;
- external C#/TypeScript adapter tests where those runtimes are available;
- the full Rust Lexicon suite.

## Related docs

- [Lexicon-wide performance restoration](lexicon-wide-performance-restoration.md)
- [Lexicon Go-path optimization parity audit](lexicon-optimization-parity-audit.md)
- [Roadmap](../planning/roadmap.md)
- [Testing and benchmarks](testing-and-benchmarks.md)

## Notes

This is a parity inventory, not a benchmark result. Dated runtime measurements belong in the owning performance-restoration or evaluation reports; this document records which architectural optimization properties must remain protected.
