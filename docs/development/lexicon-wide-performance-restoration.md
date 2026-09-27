# Lexicon-Wide Performance Restoration

Parent index: [Development Documentation](INDEX.md)

**Started:** 2026-09-26  
**Current phase:** Phase 0 complete — Lexicon-wide instrumentation and Hermes baseline  
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

The next implementation phase is **Phase 1 — restore Python bounded extraction**. The first target is the repository-wide `Repository.files: Vec<SourceFile>` lifetime: file-local read/parse/extract must produce compact fragments and release full source/AST state before deterministic bounded merge.

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
