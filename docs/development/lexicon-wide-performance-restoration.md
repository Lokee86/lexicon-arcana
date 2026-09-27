# Lexicon-Wide Performance Restoration

Parent index: [Development Documentation](INDEX.md)

**Started:** 2026-09-26  
**Current phase:** Phase 4 complete — storage/materialization copies removed; Phase 3 remains pending
**Predecessor:** [Lexicon Go-path performance restoration](lexicon-performance-restoration.md)

## Purpose

Restore the performance architecture that existed in mature pre-port Lexicon and was lost during the Rust migration without changing semantic output.

Semantic facts, identities, ownership, ordering, incremental behavior, snapshot determinism, and adapter correctness remain invariants. Performance work must reduce work, copies, cardinality, retained state, or serialization rather than prune facts.

The completed Go adapter/helper restoration remains valid evidence and implementation work, but it is not Rust-wide performance parity.

## Overview

The restoration proceeds from measurement to lifetime repair, then to ownership/materialization repair, optimization-history parity, profile-driven core work, incremental restoration, and permanent multi-language regression coverage. Hermes is the primary Python workload; secondary repositories validate that fixes preserve Lexicon-wide and cross-language behavior.

## Phase 0 — lifecycle instrumentation and Hermes baseline

Completed 2026-09-26.

Phase 0 adds opt-in `LEXICON_PERF=1` instrumentation across the Rust lifecycle:

`source inventory → change detection → scan planning → adapter discovery → extraction → repository resolution → final fact emission → canonicalization/validation → ownership partitioning → object construction/CAS → publication`.

Instrumentation is emitted only to stderr and is disabled by default.

Python-specific counters expose:

- discovered Python files and source bytes;
- retained source bytes and retained parsed files/ASTs;
- extraction-state cardinality;
- retained calls, imports, functions, classes, and binding state;
- fact counts before and after repository resolution;
- the existing full call-vector clone used by outcome emission;
- ownership partition count and whole-analysis record cloning;
- source-byte cloning during storage materialization.

The development profiler is `lexicon/examples/lifecycle_profile.rs`. The wrapper `scripts/lexicon_lifecycle_baseline.py` builds that example from the repository in release mode, uses scratch Lexicon state, samples process-tree RSS, parses lifecycle events, and records cold/warm/CAS/fact-hash metadata when a scan completes. A scan timeout records a partial profile with `classification: pathological` instead of forcing an unbounded repository benchmark to finish.

## Mature pre-port Hermes oracle

The preserved mature Python optimization record measured Hermes at **6,694 Python files / 84.3 MB** with **16 active workers / 256 logical shards / merge fan-in 8**.

The final mature result was:

- **134.574 s** cold wall time;
- **3.612 GiB** peak RSS;
- **1,058,412,847 bytes / 2,972,095 lines** canonical JSONL;
- SHA-256 `6f3744688d376e5473bbe6e5f333d63afebc2f0c9a1b95630840ae8eacd6a3a4`.

The earlier pre-optimization baseline was 285.5 s / 6.10 GiB. These measurements remain the performance oracle for the restoration; the current Hermes checkout is larger, so absolute cardinalities are not compared as though the repositories were identical.

## Rust Hermes Phase 0 baseline: pathological

Target: current `C:\!bin\workspace\hermes-agent` checkout during the Phase 0 run.

The repo-built Rust release profiler did not complete a cold scan. The scan process ran for approximately **19m10s** before termination and was still executing repository-wide resolution. Because no cold snapshot was published, there is no valid warm-scan or final canonical export/hash for this run.

That is classified as a **pathological baseline**, not a normal timing sample. It exceeded the mature 134.574 s cold baseline by more than 8× without reaching storage/publication.

The partial stage profile before termination was:

| Stage | Measurement |
| --- | ---: |
| Relevant source inventory | **140.648 s** |
| Relevant files / bytes | **11,017 / 123,089,584 B** |
| Git staging + change detection after inventory | **~30.269 s** |
| Scan planning | **147.779 ms** |
| Python discovery + parse | **58.080 s** |
| Python files / source bytes | **7,114 / 88,964,777 B** |
| Retained Python source byte+string storage | **177,929,554 B** |
| Retained parsed files / AST roots | **7,114 / 7,114** |
| Python extraction | **86.926 s** |
| Facts after local extraction/semantic emission | **2,927,306** |
| Extraction-state cardinality | **3,070,740** |
| Calls retained | **636,923** |
| Imports retained | **101,005** |
| Functions retained | **137,020** |
| Classes retained | **15,735** |
| Binding-state entries retained | **1,013,718** |
| Observed process RSS during the run | **at least 2,937,159,680 B** |

The original run nested the source inventory timer inside the initial change-detection timer. The final instrumentation separates those timers; the ~30.269 s change-detection figure above is the difference between the two recorded nested measurements.

Repository resolution had not returned by the pathological cutoff, so its completed duration and post-resolution cardinality are intentionally not fabricated.

## Phase 0 diagnosis

The profile establishes the divergence required by the Phase 0 gate.

1. **Python bounded extraction was lost.** The Rust adapter retains all 7,114 Python source buffers, duplicate source strings, and all 7,114 parsed AST roots simultaneously. The mature Python adapter explicitly released full source and file AST state before repository-wide merge/resolution.
2. **Global Python resolution state is enormous.** Before resolution completes, the Rust adapter holds roughly 3.07 million indexed/extraction-state entries, including 636,923 calls and more than one million binding-related entries.
3. **Repository resolution is itself pathological.** After discovery and extraction completed, repository-wide resolution continued for many additional minutes and never reached the next lifecycle event before termination.
4. **Lexicon-wide materialization copying remains visible in code and is now measurable.** `Analysis::groups()` clones the complete analysis into ownership groups; source materialization clones source bytes; full materialization then constructs repository-scale objects. These stages were not reached on the pathological Hermes run, but the instrumentation is in place for smaller fixtures and for the post-Phase-1/2 Hermes rerun.
5. **Source inventory/change detection is also material.** The Rust scan spent roughly 171 seconds on initial mirroring, staging, and change detection before adapter work. This is not the primary Python catastrophe, but it is already slower than the mature entire Hermes scan and remains a later core profiling target.

## Phase 0 gate

Phase 0 is complete because:

- lifecycle instrumentation covers the required Rust scan/adapter/storage boundaries;
- Python retained-state/cardinality counters expose the lost lifetime architecture directly;
- a repo-built Rust profiler measures scratch-state scans without mutating the source repository;
- the Hermes Rust baseline is demonstrably pathological and has a preserved partial stage profile rather than an invented completed result;
- the mature pre-port Hermes measurements and canonical output hash remain the oracle;
- no Phase 1 lifetime optimization or semantic change is included.

## Phase 1 — bounded Python extraction

Completed 2026-09-26.

Phase 1 restores the mature file-lifetime boundary without changing the resolver or semantic fact contract.

Python discovery now inventories compact file descriptors only: path, repository-relative path, module identity, and source size. It no longer reads, decodes, or parses every Python file up front. Bounded extraction then partitions the sorted inventory into contiguous size-weighted logical shards and executes at most the scan plan's active-worker count with scoped Rust threads.

Each file now follows one lifetime:

`read → decode → parse → file-local extraction → file-local semantic facts → retain resolver fragments → drop source bytes/string/full AST root`.

Shard results are merged strictly by shard order with the configured merge fan-in. This preserves the previous serial insertion/collision semantics while allowing independent file work to run concurrently. Repository-wide call/binding/function/class resolver state is deliberately unchanged; compacting that state is Phase 2.

Phase 1 instrumentation adds peak simultaneous source bytes, loaded files, and full AST roots. Discovery itself reports zero retained source/AST state.

### Semantic gates

The frozen Phase 0 Rust fixture remains exactly identical after the lifetime change:

- **26 facts / 6,435 JSONL bytes**;
- SHA-256 `339ccbabf1135f4c53c0d4fbd37186553f1997afc6cf4bedc2b9116ddddb5583`.

A Rust-native adapter test also analyzes the same multi-file Python repository with serial `1/1/2` execution and partitioned `3/3/2` execution and requires the complete headers and fact-record vectors to be identical.

The full Rust library suite passes **61/61** after the change.

### Validation-corpus measurement

Space Rocks `tools/` at repository revision `431625042dbdb1a884954cab6ec726413aa36e2b` provides a larger bounded check:

| Measurement | Phase 1 result |
| --- | ---: |
| Python files / source bytes | **116 / 617,758 B** |
| Discovery | **5.472 ms** |
| Discovery-retained source / files / ASTs | **0 B / 0 / 0** |
| Extraction | **2.602 s** |
| Peak extraction source retention | **45,424 B** |
| Peak loaded files / AST roots | **1 / 1** |
| Facts before resolution | **23,771** |
| Repository resolution | **647.674 ms** |
| Final facts | **30,605** |
| Whole debug scan after build | **7.043 s** |
| Canonical JSONL | **10,379,009 B / 30,606 lines** |
| SHA-256 | `04e915ba630332a3eef983a8bdac019aa5160dc8164a2b800d73da7c99423ce8` |

The normal scan planner selected one active extraction worker and two logical shards for this corpus, so the observed peak is one full file/AST. The partitioned equality test separately exercises three active workers. By construction each worker owns at most one full loaded file at a time.

Phase 1 therefore removes the repository-wide source/full-AST lifetime identified in Phase 0. It does not claim that Hermes is now fast: the Phase 0 run already localized the remaining catastrophe to multi-million-entry repository-wide resolver state, which is the explicit Phase 2 target.

## Phase 2 — compact Python global resolution state

Completed 2026-09-26.

Phase 2 restores the mature repository-resolution architecture without changing emitted facts. The Rust port had regressed several global algorithms and ownership boundaries even after Phase 1 fixed file lifetimes.

The restoration includes:

- module-suffix indexing and cached module/reference resolution instead of repeatedly scanning every module;
- reverse-dependency work-queue convergence for re-export imports instead of whole-import fixed-point rescans;
- integer-backed callgraph indexes into retained assignment/loop/import vectors instead of cloning AST-bearing records into every secondary index;
- cached class hierarchy, MRO, and descendant lookup during callgraph analysis;
- moving the call/import vectors temporarily out of `Facts` for mutating phases instead of cloning the complete vectors;
- releasing Python analysis-only indexes and AST-bearing resolver state before canonical materialization;
- focused lifecycle events for import, inheritance, override, and call resolution.

The new import regression test deliberately orders a dependent re-export before the binding it depends on and requires the work queue to revisit only the affected import while preserving the final binding.

### Semantic gates

The frozen Phase 0 fixture remains byte-identical:

- **26 facts / 6,435 JSONL bytes**;
- SHA-256 `339ccbabf1135f4c53c0d4fbd37186553f1997afc6cf4bedc2b9116ddddb5583`.

Space Rocks `tools/` also remains byte-identical to the Phase 1 result:

- **30,605 facts / 10,379,009 JSONL bytes**;
- SHA-256 `04e915ba630332a3eef983a8bdac019aa5160dc8164a2b800d73da7c99423ce8`.

On that corpus, repository resolution changed from **647.674 ms** in the Phase 1 sample to **475.159 ms** in the Phase 2 sample. Import resolution itself completed in **23.583 ms** for 829 imports. Whole-scan timing is not compared because the two runs had materially different unrelated extraction/host timing.

The final Rust library suite passes **62/62**, including the focused import re-export regression.

### Hermes Phase 2 profile

The same repo-built release profiler was run against the current Hermes checkout with a **600 s** hard scan cap. The checkout had grown to **7,114 Python files / 88,964,777 B** and produced **3,724,398** final Python facts before materialization.

The Python resolver now completes:

| Stage | Phase 2 |
| --- | ---: |
| Python extraction | **36.583 s** |
| Import resolution — 101,005 imports | **3.414 s** |
| Inheritance resolution — 15,735 classes | **180.795 ms** |
| Override resolution — 137,020 functions | **748.375 ms** |
| Call resolution — 636,923 calls | **73.905 s** |
| Total repository resolution | **78.427 s** |
| Final fact emission | **30.437 s** |
| Canonicalization | **14.274 s** |
| Validation | **4.123 s** |
| Peak process-tree RSS observed | **5,069,602,816 B** |

This is the Phase 2 success condition: Phase 0 never returned from repository resolution during its pathological run, while Phase 2 resolves the full current Hermes Python graph in approximately 78 seconds. Call resolution is now the dominant Python resolver cost rather than import convergence.

The cold scan still hit the 600-second cap, but only after Python resolution and canonical validation completed. The remaining measured pathology is the Lexicon-wide ownership/materialization path:

- ownership partitioning: **44.527 s**, cloning all **3,724,398** records;
- object construction/CAS: **36.915 s**, cloning **3,458,391** owned records again;
- publication had not completed when the cap fired;
- source inventory/change detection also remains materially expensive and is deferred to later profile-driven core work.

No final Hermes snapshot/hash is claimed because the cold scan did not publish. The timeout has moved beyond the Phase 2 target and directly exposes the Phase 3/4 work already identified in the roadmap.

### Phase 2 gate

Phase 2 is complete because:

- the multi-minute/non-terminating repository-resolution pathology is removed;
- import resolution is indexed and bounded by affected re-export dependencies;
- high-cardinality callgraph indexes no longer duplicate AST-bearing records;
- analysis-only Python state is released before canonical materialization;
- frozen fixture and Space Rocks canonical hashes remain exact;
- the remaining Hermes timeout occurs after Python resolution, at measured ownership/materialization cloning boundaries.

The next implementation phase in the original sequence is **Phase 3 — partitioned analysis core abstraction**.

## Phase 4 — storage/materialization copying removal

Completed 2026-09-26, out of sequence at the user's request. **Phase 3 remains pending.**

Phase 4 removes repository-scale payload copying from the existing flat `Analysis` materialization path without changing ownership, ordering, object bytes, or snapshot semantics.

The storage boundary now:

- partitions analysis into borrowed `&FactRecord` references instead of cloning every fact into owner/shared vectors;
- indexes source files as borrowed `&[u8]` slices instead of cloning source buffers;
- passes borrowed paths, source slices, and record slices through parallel file-write jobs;
- encodes file and shared objects directly from borrowed record selections;
- keeps the public owned `FactObject` encoder unchanged while routing both owned and borrowed inputs through the same binary-v2 implementation;
- iterates node, edge, and unresolved sections directly instead of constructing temporary section-local reference vectors.

This intentionally does **not** implement the Phase 3 partitioned-analysis abstraction. The authoritative analysis is still one flat canonical `Vec<FactRecord>`; Phase 4 only removes redundant copies made while turning that analysis into existing file/shared CAS objects.

### Phase 4 semantic and storage contracts

The following remain invariants:

- fact ordering and ownership assignment;
- binary fact-object bytes and CAS object IDs;
- source content IDs;
- file/shared object boundaries;
- incremental owner validation;
- manifest ordering and snapshot identity;
- legacy owned `FactObject` encode/decode behavior.

A focused regression encodes the same object through the normal owned-record path and the borrowed-record path and requires byte-for-byte equality.

Instrumentation now reports `record_clones = 0` during ownership partitioning and object construction, plus `cloned_source_bytes = 0`. Borrowed record-reference counts remain visible so repository scale can still be measured.

### Phase 4 semantic gates

The frozen Phase 0 fixture remains exactly unchanged after materialization:

- **26 facts / 6,435 JSONL bytes / 27 lines**;
- SHA-256 `339ccbabf1135f4c53c0d4fbd37186553f1997afc6cf4bedc2b9116ddddb5583`.

Space Rocks `tools/` also remains exactly unchanged:

- **30,605 facts / 10,379,009 JSONL bytes / 30,606 lines**;
- SHA-256 `04e915ba630332a3eef983a8bdac019aa5160dc8164a2b800d73da7c99423ce8`.

The binary regression independently verifies that owned `FactObject` input and borrowed record selections encode to byte-identical binary-v2 objects.

### Hermes Phase 4 profile

The repo-built release profiler completed the same current Hermes checkout that timed out in Phase 2. Phase 4 published a cold snapshot inside the existing 600-second scan cap.

| Measurement | Phase 2 | Phase 4 |
| --- | ---: | ---: |
| Final Python facts | **3,724,398** | **3,724,398** |
| Ownership partitioning | **44.527 s** | **3.434 s** |
| Ownership record clones | **3,724,398** | **0** |
| Object construction/CAS | **36.915 s** | **11.622 s** |
| Object-construction record clones | **3,458,391** | **0** |
| Source bytes cloned | full source copy | **0** |
| Cold scan result | timed out before publication | **published** |
| Cold scan wall | >600 s cutoff | **462.842 s** |

Ownership partitioning fell by approximately **92.3%**, and object construction/CAS by approximately **68.5%**, on the current Hermes corpus. These are the Phase 4 target stages; unrelated Python extraction/resolution differences between single runs are not attributed to Phase 4.

The completed current-revision baseline produced:

- snapshot ID `sha256:5fb52466fffc5b204990c53fe4044c5a83dd1cd94a38b2c1e64ffe1301dc57aa`;
- **3,724,398** facts;
- **1,255,784,776 bytes / 3,724,399 lines** canonical JSONL;
- SHA-256 `682e5e96e89fae2d4184dac3713491444e78654bc9e6f57bacfe8f7c470dc64f`;
- **7,118 CAS objects / 150,614,298 bytes**;
- peak process-tree RSS **5,416,800,256 B**.

The peak RSS is recorded rather than claimed as an improvement: Phase 4 removes materialization payload copies, but the observed overall process peak can occur earlier in Python analysis.

The no-change warm scan reused the same snapshot with no analysis plan in approximately **105.956 s**. Roughly **105.199 s** of that was source inventory, making source inventory/change-detection work an explicit later profile-driven target.

### Phase 4 gate

Phase 4 is complete when the isolated branch gates are green because repository-scale fact/source payload clones are removed, owned-vs-borrowed encoding is byte-identical, Hermes publishes inside the previous timeout, and Phase 3 remains an explicit separate task.

## Execution sequence

The active restoration sequence is:

**Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8.**

Phases 1–2 address the Python/Hermes catastrophe. Phases 3–4 restore Lexicon-wide partitioned ownership and bounded storage materialization. Phase 5 audits every mature pre-port optimization. Later phases are profile-driven.

## Related docs

- [Lexicon Go-path performance restoration](lexicon-performance-restoration.md)
- [Lexicon optimization parity audit](lexicon-optimization-parity-audit.md)
- [Lexicon Rust migration](../../lexicon/docs/RUST_MIGRATION.md)
- [Roadmap](../planning/roadmap.md)

## Notes

The Phase 0 Hermes Rust run is a dated pathological diagnostic, not a performance guarantee. It intentionally has no final fact hash because the cold scan never published. Semantic parity continues to be protected by the existing Rust migration and adapter correctness suites; final Hermes hash equivalence is re-established only after the pathological stages are restored enough to complete.
