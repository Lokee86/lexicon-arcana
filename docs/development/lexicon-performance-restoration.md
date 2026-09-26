# Lexicon Performance Restoration

Parent index: [Development Documentation](INDEX.md)

**Started:** 2026-09-26  
**Freeze point:** `6ac9000b462243d8c7d27748f7ef21a4e6f037ff`  
**Go adapter migration:** Phase 16 remains paused while this project is active.

## Purpose

Restore the performance characteristics lost during the Rust Go-adapter port without changing the frozen semantic contract, and retain measured evidence for each restoration phase.

## Overview

Phase 0 establishes opt-in timing/cardinality instrumentation and controlled baselines. Later phases use those measurements to restore indexed assembly, structural canonicalization, early semantic compaction, bounded semantic lifetimes, incremental execution, and measured protocol/micro-optimizations. The Go adapter migration remains paused at Phase 16 until this independent restoration project is complete.

## Contract

This project restores performance characteristics lost during the Rust Go-adapter port. The mature Go implementation is the optimization oracle.

Performance may change. Facts, identities, ownership, ordering, incremental semantics, and deterministic output may not.

Phase 0 adds measurement only. It must not optimize or redesign the adapter.

## Opt-in instrumentation

Set `LEXICON_PERF=1` for development or calibration runs.

Instrumentation is disabled by default. When enabled, measurements are written to stderr as lines beginning with:

```text
[lexicon-perf]
```

The fact stream, helper protocol payload, and stored Lexicon objects do not contain performance records.

False-like values (`0`, `false`, `off`, and `no`) disable instrumentation.

## Phase 0 stage coverage

The native Go path reports:

- repository discovery;
- structural parsing;
- `packages.Load`;
- semantic indexing;
- relationships;
- calls/dataflow;
- SSA/VTA;
- helper response encoding;
- helper IPC;
- Rust response decoding;
- fact materialization;
- dependency construction;
- canonicalization;
- final serialization/storage.

The counters are intentionally small and diagnostic rather than a general profiler. They include:

- discovered files and retained source bytes;
- loaded packages;
- typed targets and types;
- raw and compacted call observations;
- raw and compacted dataflow records;
- helper request/response bytes;
- materialized nodes and edges;
- final fact count and final serialized bytes where available.

## Baseline procedure

Phase 0 uses one controlled instrumented run for each baseline target after the helper and native adapter tests pass:

1. the small Go oracle fixture at `lexicon/adapters/go/testdata/oracle/basic_calls`;
2. Demon Docs at pinned revision `fa5ca9aea12e20c29c378d5d018647958b862cac`.

The real-repository run uses the existing Phase 16 execution plan of 4 workers, 8 logical shards, and merge fan-in 4. Do not repeatedly rerun Demon Docs merely to smooth timing noise.

Exact output parity is protected separately by the existing Go helper/native adapter tests and Phase 15 differential fixture gate. The small fixture is also run once with instrumentation disabled and once enabled so its canonical JSONL can be compared byte-for-byte.

## Baseline results

Captured 2026-09-26 with the native snapshot example built in Cargo's development profile and the current private Go helper. Both targets used 4 workers, 8 logical shards, and merge fan-in 4.

### Small fixture

Target: `lexicon/adapters/go/testdata/oracle/basic_calls`.

The instrumented run completed in approximately 3.62 seconds wall-clock.

| Stage | Time |
| --- | ---: |
| Repository discovery | 13.261 ms |
| Structural parsing | 1.067 ms |
| `packages.Load` | 1,242.766 ms |
| Semantic indexing | 0.998 ms |
| Relationships | <0.001 ms |
| Calls/dataflow | 2.344 ms |
| SSA/VTA | 740.047 ms |
| Helper response encoding | <0.001 ms |
| Helper IPC | 2,347.292 ms |
| Rust response decoding | 13.363 ms |
| Dependency construction | 2.742 ms |
| Fact materialization | 24.555 ms |
| Canonicalization | 6.797 ms |
| Final serialization/storage | 3.245 ms |

Cardinality: 3 discovered files, 472 retained source bytes, 66 loaded packages, 6 typed targets, 2 typed types, 9 raw/9 compacted calls, 12 raw/12 compacted dataflow records, 8,753 helper response bytes, 32 materialized nodes, 44 materialized edges, 77 final facts, and 22,056 final JSONL bytes.

The disabled and enabled runs produced the same canonical JSONL SHA-256:

`596A04253AA5A427EF56F374C1FF97A6929719D9401B8E15C6E5DF3D1D5FA8B7`

### Demon Docs

Target revision: `fa5ca9aea12e20c29c378d5d018647958b862cac`.

The single instrumented run completed in approximately 57.28 seconds wall-clock.

| Stage | Time |
| --- | ---: |
| Repository discovery | 7,915.867 ms |
| Structural parsing | 198.655 ms |
| `packages.Load` | 20,430.751 ms |
| Semantic indexing | 12.254 ms |
| Relationships | 2.602 ms |
| Calls/dataflow | 269.396 ms |
| SSA/VTA | 3,248.734 ms |
| Helper response encoding | 201.100 ms |
| Helper IPC | 27,173.294 ms |
| Rust response decoding | 2,699.172 ms |
| Dependency construction | 80.793 ms |
| Fact materialization | 7,086.418 ms |
| Canonicalization | 4,432.478 ms |
| Final serialization/storage | 5,037.089 ms |

Cardinality: 423 discovered files, 1,769,246 retained source bytes, 469 loaded packages, 3,489 typed targets, 337 typed types, 30,647 raw calls compacted to 20,962, 143,466 raw dataflow records with **no reduction before IPC**, 60,882,434 helper response bytes, 27,393 materialized nodes, 112,625 materialized edges, 140,018 final facts, and 50,932,343 final JSONL bytes.

These measurements identify work for later phases rather than changing it in Phase 0. In particular, the un-compacted dataflow stream and approximately 60.9 MB helper response give Phase 4 a directly measured target; the `packages.Load` cost and retained package state motivate Phase 5; JSON decoding, canonicalization, and final serialization are separately visible for Phases 2 and 7. The baseline does not by itself authorize semantic or protocol changes.

## Phase 0 exit gate

Phase 0 is complete only when:

- instrumentation is opt-in and stays off normal output streams;
- the requested stage timings and cardinality proxies are observable;
- helper and native Go adapter verification remains green;
- the Phase 15 differential fixture gate remains green;
- enabled versus disabled instrumentation produces identical canonical facts on the small fixture;
- the two baseline runs are recorded;
- no performance optimization or semantic change is included in the Phase 0 commit.

The intended commit is `Instrument Lexicon analysis performance`.

## Phase 1 — indexed fact materialization

Completed 2026-09-26.

The Go fact materializer now owns a single hash-backed `FactIndex` for node existence, node ownership, and semantic-pass edge deduplication. Node insertion updates the output vector and index together; the output `Vec<FactRecord>` is no longer used as an ownership lookup table. Existing pre-semantic nodes seed the node/owner index, while the edge index intentionally starts empty to preserve the previous semantic-pass deduplication boundary.

Production materialization no longer uses ordered `BTreeSet` membership structures. The remaining `BTreeSet` uses under `lexicon/src/adapters/go` are differential/test machinery, not the materialization hot path. Production owner recovery no longer scans `records.iter()`.

A single controlled Demon Docs run at the same pinned revision and `4 / 8 / 4` execution shape produced byte-identical canonical JSONL to the Phase 0 baseline:

`AE3064C2085AA479B058F026A26D7DC3CE2DD05EBEE036023C94E7543E650F0E`

Measured stage changes from the Phase 0 run were:

| Stage | Phase 0 | Phase 1 |
| --- | ---: | ---: |
| Dependency construction | 80.793 ms | 28.081 ms |
| Fact materialization | 7,086.418 ms | 6,635.411 ms |

Other wall-clock differences in the two single runs are treated as run-to-run noise rather than attributed to Phase 1.

The Rust library suite passed 54/54 after the change, including deterministic parallel execution and both live legacy/native differential fixture gates.

The intended commit is `Restore indexed fact materialization`.

## Phase 2 — structural fact canonicalization

Completed 2026-09-26.

Canonical sorting no longer serializes every fact into a tagged JSON object before sorting. The existing record-kind and semantic primary ordering remains unchanged; ties are now resolved structurally from the remaining record fields. Optional fields preserve the legacy JSON ordering rule that a present earlier key sorts before an omitted key. Arbitrary JSON attributes are serialized only when the primary fields and earlier structural tie-break fields are indistinguishable.

Focused comparator tests compare the new total order pairwise against the previous JSON-byte tie-break, including optional fields, attributes, source spans, and escaped string values. The full Rust library suite passed 56/56 after the change, including deterministic parallel execution and the live legacy/native differential gates.

A single controlled Demon Docs run at the same pinned revision and `4 / 8 / 4` execution shape produced byte-identical canonical JSONL to Phase 1:

`AE3064C2085AA479B058F026A26D7DC3CE2DD05EBEE036023C94E7543E650F0E`

Canonicalization time changed from **4,112.866 ms** in Phase 1 to **410.940 ms** in Phase 2, a reduction of approximately **90.0%** on the controlled development-profile run. Other stage and wall-clock differences are treated as run-to-run noise rather than attributed to Phase 2.

The intended commit is `Restore structural fact canonicalization`.

## Phase 3 — reduce retained and computed state

Completed 2026-09-26.

The Go semantic helper no longer eagerly computes and stores value/pointer method-set identity slices on every `typedType`. Relationship analysis continues to derive method sets from the retained `*types.Named` values where required. The method-set expectations in `semantic_index_test.go` are now computed locally by the test, so production state no longer exists only to satisfy test inspection.

The Rust-owned Go discovery inventory now computes each file content ID immediately after reading it. Ordinary `.go` bodies are then released rather than retained for the rest of the adapter run. `go.mod` bodies remain retained because module ownership and dependency materialization still consume the manifest text. File fact materialization uses the precomputed content ID, preserving identity without retaining source bytes.

An audit of the Rust Go-adapter inventory found no second persistent full-source-byte collection after this change. The remaining explicit `fs::read` in dependency replacement handling reads a replacement `go.mod` transiently to obtain its module path and does not retain the file body.

The full Rust library suite passed 56/56, including deterministic parallel execution and both live legacy/native differential gates. The Go semantic helper suite also passed.

A single controlled Demon Docs run at the same pinned revision and `4 / 8 / 4` execution shape produced byte-identical canonical JSONL to Phase 2:

`AE3064C2085AA479B058F026A26D7DC3CE2DD05EBEE036023C94E7543E650F0E`

The Phase 0 retained-source proxy was **1,769,246 bytes**. After Phase 3 it is **1,311 bytes**, a reduction of approximately **99.93%**. The remaining retained bytes are the repository's Go manifest content. Other timing differences in the single controlled run are treated as run-to-run noise rather than attributed to Phase 3.

The intended commits are `Remove unused semantic type state` and `Reduce Go inventory source retention`.

## Phase 4 — early semantic compaction

Completed 2026-09-26.

Direct call observations are now compacted by callsite inside each semantic shard using the existing precedence rules: resolved observations replace unresolved ones, duplicate resolved targets are removed, multi-target callsites are promoted to `possible`, and the highest-priority call class is preserved. Fan-in merges these compact callsite buckets rather than concatenating raw call slices, and the scanner no longer performs a second repository-wide direct-call compaction pass.

Dataflow observations are now keyed and deduplicated inside each shard by their complete semantic record identity: source, target, read/write kind, owner, and full source span. Fan-in merges the compact maps while preserving deterministic insertion order for structural ties. Calls and dataflow are flattened and sorted only after the shard reduction completes.

Focused compaction tests prove that cross-shard call merging matches the previous global `mergeDirectCallRecords` result and that exact duplicate dataflow observations collapse without changing deterministic ordering. The helper execution-shape test now covers `1/1/2`, `2/2/2`, `2/4/2`, `4/8/4`, and `3/6/8` worker/shard/fan-in configurations. The full Rust library suite passed 56/56, including the live legacy/native differential gates.

A single controlled Demon Docs run at the pinned revision and `4 / 8 / 4` execution shape produced byte-identical canonical JSONL to Phase 3:

`AE3064C2085AA479B058F026A26D7DC3CE2DD05EBEE036023C94E7543E650F0E`

The Phase 4 compaction measurements were:

| Measure | Before Phase 4 | Phase 4 |
| --- | ---: | ---: |
| Raw call observations | 30,647 | 30,647 |
| Compacted calls | 20,962 | 20,962 |
| Raw dataflow observations | 143,466 | 143,466 |
| Compacted dataflow observations | 143,466 | 80,895 |
| Helper response records | 172,071 | 109,500 |
| Helper response bytes | 60,882,434 | 38,908,896 |

Dataflow records sent across IPC fell by **43.61%** and helper response size fell by **36.09%**. The helper response is no longer close to the 64 MiB capture ceiling. Timing differences from this single run are not treated as stable performance claims; the retained result is the reduction in intermediate cardinality and response size.

The intended commit is `Restore early Go semantic compaction`.

## Phase 5 — bounded semantic lifetimes

Completed 2026-09-26.

The module lifetime model now matches the frozen pre-port Go optimization oracle at `c822f4d`: each module is loaded, indexed, resolved, compacted, and merged into repository state before the next module is loaded. Repository-wide state retains compact semantic records and identities only; `packages.Package`, syntax/type maps, typed target/type objects, SSA programs, and VTA state remain module-local and become unreachable after that module is merged.

The previous repository-wide semantic index and `rootGroups` retention path are gone. Module-local relationships are resolved before calls so interface implementation information remains available to typed call resolution and SSA for that module. Local module replacements/imports remain visible through the loading module's `packages.Load` graph, preserving cross-module relationships without retaining every module's compiler graph simultaneously.

A dedicated two-module regression fixture exercises a local replacement from an application module to a contracts module. It verifies:

- the application type still implements the interface declared by the contracts module;
- the interface call still resolves to the concrete application method;
- reversing module processing order produces identical compact semantic output;
- the contracts module index does not retain the unrelated application type; and
- total packages loaded across both module passes is greater than the peak number of packages live in any one pass.

Performance instrumentation now reports `processed_modules` and `peak_live_packages` alongside total `loaded_packages`, so the lifetime bound is observable on multi-module repositories.

The full Go semantic helper suite passed after the change. The Rust library suite passed 56/56, including deterministic parallel execution, incremental tests, and both live legacy/native differential gates.

A single controlled Demon Docs run at the pinned revision and `4 / 8 / 4` execution shape produced byte-identical canonical JSONL to Phase 4:

`AE3064C2085AA479B058F026A26D7DC3CE2DD05EBEE036023C94E7543E650F0E`

Demon Docs contains one Go module, so its Phase 5 profile reports **469 total loaded packages, 469 peak live packages, and 1 processed module**. That run therefore does not demonstrate a memory reduction from module bounding and is retained only as a real-repository parity check. Timing differences from the single run are likewise not attributed to Phase 5.

The intended commit is `Bound Go semantic analysis by module`.

## Phase 6 — incremental execution audit

Completed 2026-09-26.

The incremental planner, dependency topology, fallback rules, and object-reuse machinery survived the Rust port substantially intact. The audit against the frozen Go implementation at `c822f4d` found one concrete work-scope regression rather than a missing incremental subsystem.

Production incremental execution remains upstream-scoped rather than “analyze the full repository and discard most output”: the planner computes the emitted dependent closure and forward context closure from the previous stored fact graph, the scan executor builds a temporary repository containing that context, and the Go adapter analyzes that temporary repository. Additions, deletions, and Go manifest changes continue to force full analysis, matching the established contract.

The lost optimization was Go semantic-unit expansion. The mature Go scope builder and the original Rust port expanded a selected Go source file to the other `.go` files in the same package directory. Commit `16f8769` broadened that step to every Go source file in the containing module. A one-package edit could therefore turn into near-full-module discovery, parsing, `packages.Load`, semantic analysis, and SSA/VTA work before ownership filtering.

Phase 6 restores package-directory expansion. Dependency context still adds packages actually required by the stored topology, and language configuration such as `go.mod` is still copied into the temporary repository. Unrelated packages in the same module are no longer included merely because they share a module.

The audit matrix is now protected by tests:

| Change | Established execution |
| --- | --- |
| No repository change | No adapter analysis is planned. |
| Modified implementation file | Dependency-scoped incremental analysis; the selected package is expanded to sibling Go files, not the whole module. |
| Modified API with known dependents | Reverse dependents are emitted and their forward context is included; unrelated packages remain outside the scope when the stored topology makes scoped analysis safe. |
| Added Go source | Full Go analysis. |
| Deleted Go source | Full Go analysis. |
| Modified `go.mod` | Full Go analysis. |

A real-helper regression fixture also compares the changed file's owned fact group from package-scoped incremental analysis with the same owned group from full-repository analysis. They are identical. Ownerless shared facts are intentionally not compared as newly generated incremental state: both the mature Go scan engine and the Rust scan executor preserve the previous shared object during ordinary incremental materialization rather than replacing it from the scoped repository. Existing materialization and scan-execution tests protect that reuse contract.

The Go semantic helper suite passed. The Rust library suite passed 57/57, including deterministic execution, incremental ownership, and both live legacy/native differential gates. The focused scan integration suites for planning, scope construction, materialization, and execution also passed.

Phase 6 uses invalidated work cardinality as the regression measure rather than wall-clock thresholds: no-op scans schedule zero adapter work, scoped edits name only dependency-derived context, and the scope fixture proves that an unrelated package in the same Go module is absent from the temporary analysis repository. This makes the performance contract deterministic and avoids timing-noise assertions.

The intended commit is `Restore Go incremental package scoping`.

## Phase 7 — protocol conversion overhead

Completed 2026-09-26.

The Rust helper runner no longer decodes a helper response into `serde_json::Value` and then converts that generic JSON tree into the typed protocol response. `run_json` now deserializes directly from the response frame into the typed response exactly once. A small `ProtocolResponse` interface exposes the already-deserialized protocol version so the helper runner can preserve its handshake validation before returning the response.

The wire protocol, response framing, response-size limit, typed Go response shape, and protocol mismatch behaviour remain unchanged. The redundant debug assertion in the Go adapter was removed because successful `run_json` completion now already guarantees that the typed response carries the expected protocol version.

Controlled measurements used the existing development-profile snapshot harness with the `4 / 8 / 4` worker/shard/fan-in shape.

### Small fixture

Target: `lexicon/adapters/go/testdata/oracle/basic_calls`.

- helper response: **8,753 bytes**;
- helper response encoding: **<0.001 ms**;
- Rust typed response decode: **0.915 ms**;
- canonical JSONL: **22,056 bytes**;
- SHA-256: `596A04253AA5A427EF56F374C1FF97A6929719D9401B8E15C6E5DF3D1D5FA8B7`.

The canonical output hash is identical to the Phase 0 baseline.

### Demon Docs

Target revision: `fa5ca9aea12e20c29c378d5d018647958b862cac`.

- helper response: **38,738,960 bytes**;
- helper response encoding: **126.590 ms**;
- Rust typed response decode: **812.567 ms**;
- helper IPC: **22,538.686 ms**;
- final JSONL: **50,932,343 bytes**;
- SHA-256: `AE3064C2085AA479B058F026A26D7DC3CE2DD05EBEE036023C94E7543E650F0E`.

The canonical output hash is identical to Phases 1–5. The response remains a single bounded frame, so the observed peak response payload for this run is approximately **38.74 MB**, comfortably below the existing 64 MiB response cap after Phase 4 compaction.

At the current payload size, JSON decoding accounts for roughly **0.81 s** of a **22.54 s** helper IPC interval. That cost is visible but no longer large enough to justify introducing streamed records, new framing, or a private binary protocol as part of this restoration phase. Any future protocol redesign should therefore be justified by a new profile showing protocol handling has again become material.

The intended commit is `Avoid duplicate helper response decoding`.

## Related docs

- [Go adapter Phase 16 freeze](go-adapter-port-freeze-2026-09-26.md)
- [Testing and benchmarks](testing-and-benchmarks.md)
- [Lexicon Rust migration](../../lexicon/docs/RUST_MIGRATION.md)

## Notes

The measurements above are dated development evidence, not user-facing performance guarantees. Later restoration phases must re-measure before deciding whether a measured cost still warrants optimization.
