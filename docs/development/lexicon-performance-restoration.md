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

## Related docs

- [Go adapter Phase 16 freeze](go-adapter-port-freeze-2026-09-26.md)
- [Testing and benchmarks](testing-and-benchmarks.md)
- [Lexicon Rust migration](../../lexicon/docs/RUST_MIGRATION.md)

## Notes

The measurements above are dated development evidence, not user-facing performance guarantees. Later restoration phases must re-measure before deciding whether a measured cost still warrants optimization.
