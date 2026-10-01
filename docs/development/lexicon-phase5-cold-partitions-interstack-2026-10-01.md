# Lexicon Phase 5 — Bounded cold index and interstack optimisation

Parent index: [Development Documentation](INDEX.md)
Date: 2026-10-01
Status: Verified **302-file diagnostic improvement**. Complete pinned-Hermes Phase 5 acceptance remains OPEN.

## Purpose

Reduce two independently measured native cold-scan costs without changing index ownership, immutable publication, contract-detection semantics or exported Python facts. Retain the original Phase 5 acceptance gates rather than substituting a small fixture for the full repository.

## Overview

On a pinned Hermes `agent/` subset, full index construction synchronised hundreds of content-addressed partition files in series. Interstack repeatedly queried callable ownership on ordinary lines that could not contain supported contracts. This checkpoint changes only the internal scheduling of immutable writes and skips those unneeded owner lookups.

### Immutable dependency-index publication

`storage/dependency_index_build.rs::write_shards` now publishes independent immutable partitions using up to eight scoped threads. The same store-owned CAS writer performs each hash, write and durability sync, with bounded workers and deterministic partition ordering when results are collected. The root is written only after every partition has succeeded; snapshot publication and PENDING recovery still verify every referenced partition before advancing CURRENT. A failed worker returns a materialization error; an interrupted write can leave only unreachable immutable objects for normal GC.

Existing delta and legacy-bootstrap callers still use the same writer and topology schema; no new mutable owner or snapshot format was introduced. The new `dependency_parallel_determinism` regression builds and publishes the same language in independent stores and checks identical language entries, snapshot IDs and scope results. Existing corruption, exact-generation, delta and legacy-upgrade tests also remain green.

### Interstack owner lookups

`interstack/messages.rs`, `config.rs` and `http_producers.rs` now check for a plausible message token, configuration prefix or return statement **before** searching callable-owner intervals. This preserves all candidate patterns and leaves the existing producer/consumer passes in their original order. The interstack boundary, build, resolver and route suites continue to verify detection behaviour.

`interstack/resolver.rs` additionally records opt-in per-pass timings for constant collection, consumers, HTTP providers and each producer/config/process/state detector. The metrics are observational; no new scanner, cache or ownership path was introduced.

## Paired pinned-source production CLI diagnostic

Both binaries ran in sequence on identical disposable Git archives of pinned Hermes commit `7798df0a83721df7ff44b9ba023d56b85b351d1e`, restricted to `agent/` (302 Python files, 6,302,285 bytes). The baseline binary SHA-256 begins `8f3b5b8b`; the optimised binary SHA-256 begins `0f93d37a`. Both completed cold `init`, published a snapshot and exported Python facts.

| Measured stage | Baseline | Optimised |
| --- | ---: | ---: |
| Cold CLI `init` wall | 23.625 s | 21.250 s |
| Index construction | 3.304 s | 2.267 s |
| Interstack contract detection | 5.052 s | 4.046 s |
| Peak sampled process-tree RSS | 385,884,160 B | 401,485,824 B |
| Python exported facts | 249,297 | 249,297 |

The canonical exported Python fact hashes match exactly: `d3898c6d6f736352dd8067a31c162f03c3dc3fa1dd4a5d1507c4105a6325c090`. Each interstack linking stage reports 56 nodes, 76 edges and 34 unresolved records; **interstack fact-by-fact identity was not independently exported and is not claimed here**. The new writer emitted five independently measured shard groups, 318 total partitions, with eight bounded workers. These are single-run diagnostics; do not generalise their percentage differences into statistically established hardware performance.

A historical run of the *same baseline binary* completed this subset in 39.188 seconds under a different machine workload. The paired 23.625-second baseline, rather than the unpaired historical wall time, is the appropriate comparison. The optimiser raises sampled peak RSS by about 15.6 MB on this pair; full-source memory and cold time remain unmeasured.

Raw paired results: `lexicon/evaluation/performance/incremental-phase5-agent-baseline-pair-2026-10-01.json` and `incremental-phase5-agent-optimized-pair-2026-10-01.json`.

## Detector-level follow-up profile

A separate opt-in profiling build on the exact same source revision and fixture exported the same 249,297 Python facts. Its stage values are **not** a third member of the paired speed comparison; their purpose is to identify remaining work.

| Detector group | Measured time |
| --- | ---: |
| Constants | 697 ms |
| HTTP and message consumers | 1,000 ms |
| HTTP path providers | 218 ms |
| HTTP producers | 881 ms |
| Message producers | 811 ms |
| Config reads and boundary config | 357 ms |
| Process and state contracts | 335 ms |

The profiling run measured 4.305 seconds in overall contract detection. Costs are spread across multiple file/line passes, so there is no single remaining isolated slow detector. An optional subsequent refactor could evaluate one shared lexical candidate pass, but only with independently checked interstack fact parity; do not replace the existing detectors or loosen their matching rules without that proof.

Raw detector profile: `lexicon/evaluation/performance/incremental-phase5-agent-detectors-profile-2026-10-01.json`.

## Callable ownership interval follow-up

A further implementation replaced `interstack::SourceIndex::owner_at`'s repeated linear scan of the file's sorted callable list with a single immutable, per-file maximum-end interval index. A binary search limits candidates to callables already started; a segment-tree query returns the original **first containing callable**, otherwise the last started callable or the file fallback. It preserves the original overlapping/nested-span precedence, without a second mutable graph owner. A unit test checks every line over nested, overlapping, tied-start, gapped, empty and reversed-length callable fixtures against the original scan. The existing interstack and dependency-index integration suites remain green.

A separate capped cold run on the **same pinned 302-file `agent/` fixture** completed and again exported **249,297** canonical Python facts, with the same exact SHA-256 hash as both preceding builds. This was another single run under potentially varying load, not a simultaneously paired statistical experiment:

| Measurement | Previous filtered-detector build | Interval-index build |
| --- | ---: | ---: |
| Complete cold CLI | 21.250 s | 20.813 s |
| Interstack contract detection | 4.046 s | 3.196 s |
| Total interstack processing | 4.693 s | 3.709 s |
| Index construction | 2.267 s | 2.560 s |
| Peak sampled process-tree RSS | 401,485,824 B | 389,947,392 B |

The index-construction timing increased on the interval-index run despite unchanged index logic, illustrating why individual development-profile timings must not become release thresholds. The stage evidence is consistent with fewer ownership lookups, and exported Python facts remain byte-identical. This does **not** establish independent exact interstack fact parity or complete-Hermes latency acceptance.

Raw evidence: `lexicon/evaluation/performance/incremental-phase5-agent-owner-index-2026-10-01.json`. The other two comparison runs and detector-level profile are retained above.

## Verification and release boundary

`incremental_phase5_cold_pair_gate.py` rejects mismatched revision/fixture, censored runs, absent RSS evidence, divergent Python facts, changed interstack cardinality, missing eight-worker shard measurements and missing per-detector profile. Its ten evidence-gate tests pass, including the indexed callable-ownership fixture, deliberately mismatched facts, and altered interstack cardinality. Existing targeted tests cover topology corruption and same-snapshot verification, deterministic legacy bootstrap, scoped delta, and interstack contract fixtures; an additional independent-store determinism test checks the parallel writer.

This is a bounded cold **diagnostic**, not a completed Phase 5 release gate. The full pinned Hermes source contains 7,114 Python files, and the original full cold run was cancelled at its 240-second limit. Do not claim a full-source before/after improvement, sub-five-second warm scans or complete interstack semantic parity from this subset. Before rerunning the full corpus, assess scaling on another bounded sample and obtain exact interstack export parity if changing the detector architecture.

## Related docs

- [Phase 5 shared-fact and indexed-location repair](lexicon-incremental-phase5-repairs-2026-10-01.md)
- [Initial blocked Phase 5 acceptance](lexicon-incremental-phase5-2026-10-01.md)
- [Hard-cut performance plan](../planning/lexicon-incremental-scan-performance.md)
- [Testing and benchmarks](testing-and-benchmarks.md)

## Notes

Every reported duration uses the owning stage's elapsed timer. Some stages nest inside full materialization or interstack resolution and must not be added together. The exact baseline and optimised binary hashes, environment, source revision, timeouts and measured events are retained in the JSON evidence.