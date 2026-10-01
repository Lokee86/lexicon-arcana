# Lexicon Phase 5 — Bounded Scaling and Publication Diagnosis

Parent index: [Development Documentation](INDEX.md)

## Purpose

Record the next pinned-Hermes scaling boundary and instrument publication without mistaking a censored cold run or a successful smaller fixture for full Phase 5 acceptance.

## Overview

Both samples use the pinned Hermes commit `7798df0a83721df7ff44b9ba023d56b85b351d1e` and a development Rust CLI on Windows. Each was prepared in a disposable Git fixture without modifying the active Hermes checkout. The 1,000-file run uses the first sorted pinned Python files and was censored; the completed 544-file run uses the directly archived `hermes_cli/` subtree. They are **different source populations**, not a before/after performance pair.

### Censored 1,000-file fixture

Whole-tree archive preparation and pruning took approximately 61 seconds **outside** the measured cold subprocess. Its actual cold `init` then timed out at **101.141 seconds**, with sampled peak process-tree RSS **1,304,735,744 bytes** (below the configured cap). No snapshot or exported semantic oracle was completed.

Its last completed opt-in stage was `interstack.total`. Completed work included:

| Stage | Elapsed |
| --- | ---: |
| Source mirror: 4,903 tracked files, 55,254,592 bytes | 16.042 s |
| Full Python adapter | 33.995 s |
| Full materialization, including index | 13.627 s |
| Dependency-index construction (nested within materialization) | 6.291 s |
| Interstack total | 12.389 s |

The adapter emitted 866,949 Python facts for the selected source files, but the timed-out run did not export/publish a verified snapshot. Because the binary predated the finer publication timers, **the exact timeout operation is unknown**. Avoid summing nested stages, extrapolating directly to 7,114 files, or attributing the entire remaining wall time to publication.

Raw evidence: `lexicon/evaluation/performance/incremental-phase5-hermes1000-scale-2026-10-01.json`. The complete-tree fixture setup itself is too expensive for repeated iterative profiling; use pinned path-scoped archives until the benchmark's subset preparation is improved.

### Completed 544-file pinned subtree

A smaller but larger-than-agent `hermes_cli/` sample has 544 Python files and 11,125,548 source bytes. It completed full publication, an unchanged scan and a comment-only edit with the newly instrumented publication stages.

| Operation | CLI wall | Sampled peak process-tree RSS |
| --- | ---: | ---: |
| Cold initialization | 35.406 s | 704,073,728 B |
| Unchanged scan | 1.860 s | 23,474,176 B |
| One-file edit | 12.531 s | 134,971,392 B |

The initial and unchanged exports have the same exact 446,781 Python facts and canonical hash. The edited export retains the same fact count but changes hash as expected with source content; it **has not** been independently compared with a full analysis of that edited source. This is not complete semantic acceptance.

Cold publication-stage measurements were: Git stage-all **1.887 s**, state check **0.077 s**, pending marker **0.009 s**, Git commit **1.372 s** and immutable snapshot verification **1.235 s**. These stages are now independently timed, with explicit start markers so a timeout identifies the operation in progress.

An unchanged scan skipped all 544 source file bodies; the mirror inventory/index occupied approximately 1.15 seconds within the 1.86-second scan. Its cold and warm metadata inventory costs remain worth watching on full Hermes.

The edited scan processed two Python files in its scoped adapter (0.332 s) and read **zero** unrelated fact objects in the dependency query. Its main costs were:

| Edited scan stage | Elapsed |
| --- | ---: |
| Source mirror inventory | 0.846 s |
| Shared-fact merge and dependency-index maintenance (including nested work) | 2.701 s |
| Interstack refresh (all 544 source files; **545 fact objects**) | 6.008 s |
| Git staging, commit and verified snapshot publication | 1.788 s |

The index query loaded two partitions; its delta loaded 189 of the available partition entries and rewrote 22. Shared reconciliation loaded three directly related fact objects. The full interstack refresh defeats the bounded warm path despite the properly scoped Python adapter, causing the edited scan to exceed the **10-second** requirement on this subset.

Raw evidence: `lexicon/evaluation/performance/incremental-phase5-hermes-cli-publication-2026-10-01.json`.

## Verification

Publication stage instrumentation is opt-in and does not alter the Git/pending/snapshot transaction order. Formatting, both scan-engine regressions and four publication/recovery tests passed. The earlier immutable-index, interstack and canonical Python fact parity tests remain documented in [the cold-path checkpoint](lexicon-phase5-cold-partitions-interstack-2026-10-01.md).

**Phase 5 release acceptance remains open.** This sample identifies a new first-class interstack ownership/performance problem. There is no evidence here that the original full-Hermes cold run now completes, that a full-Hermes warm edit meets its target, or that the 544-file edit matches an independent full semantic oracle.

## Related docs

- [Interstack delta-refresh repair design](../planning/lexicon-interstack-delta-refresh.md)
- [Incremental performance hard-cut plan](../planning/lexicon-incremental-scan-performance.md)
- [Cold index and interstack checkpoint](lexicon-phase5-cold-partitions-interstack-2026-10-01.md)
- [Testing and benchmarks](testing-and-benchmarks.md)

## Notes

Retain the censored 1,000-file run with its explicit timeout. Do not relabel it as a successful benchmark or rerun it without improving the full-tree fixture preparation and identifying an appropriate resource budget.
