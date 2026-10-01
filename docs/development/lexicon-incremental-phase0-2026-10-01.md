# Lexicon incremental scan: Phase 0 baseline

Parent index: [Development Documentation](INDEX.md)
Date: 2026-10-01
Branch: `perf/lexicon-incremental-scan`
Scope: instrumentation and measurement only. No scope, analysis, storage, or publication semantics were intentionally altered.

## Purpose

Measure the entire warm Lexicon production scan, distinguishing expensive setup from actual bounded incremental analysis without changing scanning semantics.

## Overview

The instrumented CLI and disposable benchmark harness establish repeatable synthetic evidence and a deliberately bounded pinned-Hermes attempt. The first real Hermes attempt timed out before publication, so full Hermes acceptance remains outstanding.

## Why this baseline is needed

Existing unit/fixture tests established the **correct affected files**, not that finding those files was cheap. The current mirror skips no source bodies in a measured clean fixture, and the current dependency planner loads the full prior language corpus before calculating a bounded impact set.

The diagnostic output is opt-in through `LEXICON_PERF=1`. Timers are scoped to discovery, source index/hash/copy, Git change detection, dependency reconstruction and selection, scoped workspace construction, adapter execution, materialization, publication, lock wait, and scan total. The disposable harness captures wall time and sampled process-tree RSS separately; child RSS requires `psutil` and is explicitly labelled.

## Reproducible harness

`lexicon/evaluation/performance/incremental_phase0.py` creates a temporary source checkout. With no `--source`, it generates a deterministic synthetic Python chain; with `--source`, it archives the given Git repository at HEAD, never including its working tree edits. It runs the production `lexicon init/scan` command over clean, edited-twice, deliberately dirty private mirror, add/delete and rename cases, preserving structured stage evidence. `--max-steps`, `--cold-timeout`, `--timeout` and optional `--interrupt` bound expensive/incomplete runs. The script deletes its disposable source/tree on exit and records no changes to the caller's checkout.

Example: pass the freshly built Lexicon binary to `--binary`, the current `lexicon/adapters` folder to `--adapters`, and an output JSON destination. For Hermes, pass `--source` pointing at an existing Hermes Git checkout. The current scanner's full Python adapter duration is reported separately from total cold initialization; do not substitute one for the other.

## First controlled baseline: synthetic Python (60 modules + initializer)

Source: synthetic Git-independent fixture, 61 indexed Python files, approximately 2.47–2.52 KB.
Binary: instrumented development-profile Rust Lexicon build from the Phase 0 worktree (initial sample before the final hash-failure diagnostic was added).
Raw structured evidence: `lexicon/evaluation/performance/incremental-phase0-synthetic-2026-10-01.json`.

| Operation | End-to-end wall | Source inventory | File-object reads for dependency rebuild | Scoped Python adapter |
| --- | ---: | ---: | ---: | ---: |
| Initial full build | 1.610 s | 382.631 ms | n/a | 36.425 ms full |
| No change | 1.047 s | 234.026 ms | n/a | none |
| One-file edit | 1.390 s | 199.533 ms | **62** | 3.550 ms incremental |
| Second one-file edit | 1.641 s | 226.527 ms | **62** | 5.369 ms incremental |
| Dirty private mirror; source unchanged | 0.938 s | 61.993 ms | n/a | none |

The private mirror was reported **clean** on both one-file scans, yet both had **zero indexed skips and 60 byte-equal comparisons** plus one copy. The dirty-private-mirror case correctly reported dirty and repaired one private file. One-file dependency planning decoded all **61 prior file objects plus the shared object** in both edited runs; the output scope contained only one file. On this small fixture, incremental *adapter* analysis took 3.5–5.4 ms, while full-production-scan overhead dominated. The small data does not establish a Hermes timing target or represent the cost of a full cold pipeline on Hermes.

## Larger synthetic scaling sample (1,000 modules + initializer)

This second run used the same instrumented development binary after adding explicit hash-failure, candidate-byte and materialization counters. The 1,001-file fixture held 43.75–43.79 KB of Python source.

| Operation | Wall time | Source inventory | Source-body comparisons | Dependency object reads | Adapter analysis |
| --- | ---: | ---: | ---: | ---: | ---: |
| Initial full build | 11.625 s | 2,736.336 ms | initial copy of 1,001 files | n/a | 611.268 ms full |
| No change | 1.453 s | 436.587 ms | 1,001 | n/a | none |
| One-file edit | 3.125 s | 402.998 ms | 1,000 + one changed copy | **1,002** | 4.772 ms incremental |
| Second one-file edit | 3.532 s | 528.320 ms | 1,000 + one changed copy | **1,002** | 3.767 ms incremental |

The **clean-mirror Git hash call failed** in all four runs, even with a successful private-Git clean-state check. It attempted to hash 1,001 files (about 43.8 KB) before returning an unusable result and falling back. This identifies a second investigation inside Phase 1: resolve the hash subprocess failure before replacing the mirror algorithm. Planning decoded every one of the 1,001 file objects plus the shared object and took **415.916 ms** and **458.321 ms** in the two edited runs, respectively. The selected incremental Python adapter consumed only 3.8–4.8 ms. These are development-profile, small-byte synthetic figures, not a promise of Hermes wall-clock performance.

Evidence: `lexicon/evaluation/performance/incremental-phase0-synthetic1000-2026-10-01.json`.

## Larger synthetic scaling sample (1,000 modules + initializer)

This second run used the same instrumented development binary after adding explicit hash-failure, candidate-byte and materialization counters. The 1,001-file fixture held 43.75–43.79 KB of Python source.

| Operation | Wall time | Source inventory | Source-body comparisons | Dependency object reads | Adapter analysis |
| --- | ---: | ---: | ---: | ---: | ---: |
| Initial full build | 11.625 s | 2,736.336 ms | initial copy of 1,001 files | n/a | 611.268 ms full |
| No change | 1.453 s | 436.587 ms | 1,001 | n/a | none |
| One-file edit | 3.125 s | 402.998 ms | 1,000 + one changed copy | **1,002** | 4.772 ms incremental |
| Second one-file edit | 3.532 s | 528.320 ms | 1,000 + one changed copy | **1,002** | 3.767 ms incremental |

The **clean-mirror Git hash call failed** in all four runs, even with a successful private-Git clean-state check. It attempted to hash 1,001 files (about 43.8 KB) before returning an unusable result and falling back. This identifies a second investigation inside Phase 1: resolve the hash subprocess failure before replacing the mirror algorithm. Planning decoded every one of the 1,001 file objects plus the shared object and took **415.916 ms** and **458.321 ms** in the two edited runs, respectively. The selected incremental Python adapter consumed only 3.8–4.8 ms. These are development-profile, small-byte synthetic figures, not a promise of Hermes wall-clock performance.

Evidence: `lexicon/evaluation/performance/incremental-phase0-synthetic1000-2026-10-01.json`.

## Larger synthetic scaling sample (1,000 modules + initializer)

This second run used the same instrumented development binary after adding explicit hash-failure, candidate-byte and materialization counters. The 1,001-file fixture held 43.75–43.79 KB of Python source.

| Operation | Wall time | Source inventory | Source-body comparisons | Dependency object reads | Adapter analysis |
| --- | ---: | ---: | ---: | ---: | ---: |
| Initial full build | 11.625 s | 2,736.336 ms | initial copy of 1,001 files | n/a | 611.268 ms full |
| No change | 1.453 s | 436.587 ms | 1,001 | n/a | none |
| One-file edit | 3.125 s | 402.998 ms | 1,000 + one changed copy | **1,002** | 4.772 ms incremental |
| Second one-file edit | 3.532 s | 528.320 ms | 1,000 + one changed copy | **1,002** | 3.767 ms incremental |

The **clean-mirror Git hash call failed** in all four runs, even with a successful private-Git clean-state check. It attempted to hash 1,001 files (about 43.8 KB) before returning an unusable result and falling back. This identifies a second investigation inside Phase 1: resolve the hash subprocess failure before replacing the mirror algorithm. Planning decoded every one of the 1,001 file objects plus the shared object and took **415.916 ms** and **458.321 ms** in the two edited runs, respectively. The selected incremental Python adapter consumed only 3.8–4.8 ms. These are development-profile, small-byte synthetic figures, not a promise of Hermes wall-clock performance.

Evidence: `lexicon/evaluation/performance/incremental-phase0-synthetic1000-2026-10-01.json`.

## Larger synthetic scaling sample (1,000 modules + initializer)

This second run used the same instrumented development binary after adding explicit hash-failure, candidate-byte and materialization counters. The 1,001-file fixture held 43.75–43.79 KB of Python source.

| Operation | Wall time | Source inventory | Source-body comparisons | Dependency object reads | Adapter analysis |
| --- | ---: | ---: | ---: | ---: | ---: |
| Initial full build | 11.625 s | 2,736.336 ms | initial copy of 1,001 files | n/a | 611.268 ms full |
| No change | 1.453 s | 436.587 ms | 1,001 | n/a | none |
| One-file edit | 3.125 s | 402.998 ms | 1,000 + one changed copy | **1,002** | 4.772 ms incremental |
| Second one-file edit | 3.532 s | 528.320 ms | 1,000 + one changed copy | **1,002** | 3.767 ms incremental |

The **clean-mirror Git hash call failed** in all four runs, even with a successful private-Git clean-state check. It attempted to hash all 1,001 files (about 43.8 KB) before returning an unusable result and falling back. Phase 1 must determine and fix this failure before discarding the Git-backed optimization. Planning decoded every one of the 1,001 file objects plus the shared object, taking **415.916 ms** and **458.321 ms** in the edited runs. The selected incremental Python adapter took only 3.8–4.8 ms. These figures are from a development-profile binary and are not Hermes timing targets.

Evidence: `lexicon/evaluation/performance/incremental-phase0-synthetic1000-2026-10-01.json`.

## Bounded pinned-Hermes production sample (incomplete cold run)

The disposable harness archived `C:\\!bin\\workspace\\hermes-agent` at **Git revision `7798df0a83721df7ff44b9ba023d56b85b351d1e`**. The active worktree and its pre-existing Lexicon/Arcana state were untouched. Only Python analysis was enabled. The Rust CLI development binary and its code revision are recorded in the evidence.

The initial `lexicon init` exceeded its deliberate **120-second timeout** (121.579 s measured including termination); sampled parent-plus-child peak RSS was **1,153,789,952 bytes** (~1.07 GiB). No snapshot was published. The captured stages before termination:

- Source discovery: 11,017 files / 123,089,584 source bytes, 621.438 ms.
- Clean mirror reported by Git; Git hash fast path failed while requesting all 11,017 files / 123 MB, 397.691 ms.
- Initial source copy: 11,017 files / 123 MB, **34,275.704 ms**; entire inventory **36,044.907 ms**.
- Python adapter discovery: 7,114 Python files / 88,964,777 bytes, **1,405.085 ms**. No later adapter completion or publication counter was recorded.

This is **right-censored evidence**, not a full cold baseline, incremental baseline, or an apples-to-apples comparison with the historical September Hermes run. The harness terminated the subprocess tree on timeout and discarded its temporary checkout. Do not increase synchronous refactor-tool timeouts to compensate; Phase 1/2 work can be based on the reproducible scope and mirror evidence while full pinned-Hermes acceptance remains a Phase 5 gate.

Evidence: `lexicon/evaluation/performance/incremental-phase0-hermes-2026-10-01.json`.

## Prior Hermes evidence (historical, not a current comparative baseline)

An earlier production-path diagnostic on the active Phase 5.8 Hermes tree reported 12,819 discovered files, about 135 MB of source, 12,807 byte-equal comparisons, zero indexed skips, 12 copies and about 22 seconds in source inventory. Git change detection found 57 changed paths. Dependency planning remained uncompleted when the diagnostic was cancelled and process private memory exceeded 2.5 GB. The exact revision and controlled full-vs-incremental pair were not pinned during that diagnostic. **Do not treat it as a controlled current Hermes benchmark.** Never run profiling against an active worktree with uncommitted Hermes refactor changes; the disposable harness archives a pinned HEAD.

## Test and measurement gates

- `cargo check --lib`: passed on instrumented sources before the final additional counters.
- Four focused integration suites (source mirror, dependency topology, scan planner and scan engine): 15/15 passed on instrumented sources.
- Before enabling the Phase 1 design, inspect the new clean-mirror hash-failure diagnostic, repeat the fixture on a larger unrelated corpus, and capture a bounded fresh pinned-Hermes sample where feasible.
- Recheck semantic and snapshot equivalence with instrumentation disabled/enabled. Source metadata counters are collected only with `LEXICON_PERF` to avoid ordinary instrumentation overhead.
- If a production Hermes scan cannot finish within its bounded diagnostic limit, retain the partial trace and mark the exact missing baseline; do not silently compare an incomplete incremental scan with full-adapter timings.

## Code map

| Boundary | Implementation and evidence |
| --- | --- |
| Timing/opt-in contract | `lexicon/src/perf.rs`, existing adapter/Interstack counters |
| Source inventory/index/copy | `lexicon/src/repository/mirror.rs`, `mirror_index.rs` |
| Dependency reconstruction/scope | `lexicon/src/storage/dependency.rs`, `scan/planner.rs` |
| Adapter, materialization and publication | `lexicon/src/scan/analysis_run.rs`, `execute.rs`, `engine.rs` |
| Reproducible production harness | `lexicon/evaluation/performance/incremental_phase0*.py`, baseline JSON |
| Correctness guards | `lexicon/tests/source_mirror.rs`, `dependency_topology.rs`, `scan_planning.rs`, `scan_engine.rs` |

Next phases remain separate: Phase 1 repairs source detection; Phases 2–4 install and maintain the persistent snapshot-scoped dependency index and remove full-object planning.

## Related docs

- [Incremental scan repair specification](../planning/lexicon-incremental-scan-performance.md)
- [Lexicon developer verification](../../lexicon/docs/DEVELOPMENT.md)
- [Testing and benchmarks](testing-and-benchmarks.md)

## Notes

Synthetic runs are development-profile diagnostic samples. Historical Hermes data is not a pinned comparator. The pinned-Hermes sample is explicitly right-censored at its cold-run timeout.
