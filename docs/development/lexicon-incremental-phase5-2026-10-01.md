# Lexicon incremental scan Phase 5 — Pinned-Hermes acceptance attempt

Parent index: [Development Documentation](INDEX.md)
Date: 2026-10-01
**Acceptance: NOT PASSED.** Preserve this report and its exact raw evidence; do not treat synthetic or package-level results as a full-Hermes release sign-off.

## Purpose

Test the Phase 0–4 performance repair through actual Rust CLI scans, not isolated planner microbenchmarks. Require exact pinned source, measured before/after results, exported-fact comparison, bounded resource usage, and lock release after interruption.

## Overview

Both instrumented **development-profile** binaries were built on the same Windows machine (16 logical CPUs, 16 GiB RAM). The before binary is Lexicon `2d896f6` (Phase 0); the after binary is `642011f` (Phase 4). The exact Hermes source is Git HEAD `7798df0a83721df7ff44b9ba023d56b85b351d1e`; active Hermes working-tree changes were never copied or modified. Every benchmark uses an isolated disposable Git source checkout. RSS samples include subprocess descendants, with a 2,250 MiB cap, per-step timeouts and Windows process-tree termination.

The complete Hermes cold scan failed its bounded run. An intermediate 320-Python-file Hermes-derived checkout also failed to complete cold initialization on the older binary. A deliberately narrow, pinned 14-file `acp_adapter` package then completed the same nine before/after scenarios, exposing a shared-fact fallback and a semantic mismatch. The complete Hermes acceptance gate remains open.

## Full pinned Hermes: incomplete cold run

| Measurement | Repaired Lexicon |
| --- | ---: |
| Relevant source files | 11,017 |
| Source bytes | 123,089,584 |
| Source synchronization | 42.228 s |
| Python files discovered | 7,114 |
| Python source bytes | 88,964,777 |
| Python adapter discovery | 3.170 s |
| Cold-scan wall before cancellation | 242.829 s |
| Configured cold limit | 240 s |
| Peak sampled RSS | 1,263,968,256 bytes (~1.18 GiB) |
| Published snapshot | **None** |

The last completed adapter stage was Python discovery; extraction/analysis had not emitted a completion stage before timeout. This is evidence of a cold-analysis bottleneck *after discovery*, not a completed measurement of which extraction substage dominates. Source synchronization is measured separately; temporary source checkout preparation is excluded from CLI wall time. A historical Phase 0 Hermes attempt reached its separate 120-second cap, but there is no new same-cap completed full-Hermes before/after pair. Do not infer a speedup from these censored runs.

The older binary's separate 320-Python-file Hermes-derived cold attempt also exceeded its 155-second limit (157.313 s including termination), peaking at 271,556,608 bytes. It is neither the full Hermes baseline nor a completed subset comparison.

## Paired pinned Hermes package — diagnostic only

The package fixture archives only `acp_adapter/` from the same Hermes commit: 14 Python files, initially 11,011 exported fact records. Both binaries ran identical source edits, no-change scans, deliberate private-mirror damage, a ten-file comment edit, add/delete/rename, an interrupted scan and subsequent recovery. Facts were exported after every completed normal scan; comparison hashes the canonical JSONL fact-record lines without the repository-specific header.

| CLI operation | Before, s | After, s | Exported facts agree? |
| --- | ---: | ---: | :---: |
| Full cold initialization | 16.891 | 12.484 | Yes |
| No change | 2.390 | 2.672 | Yes |
| One-file comment edit | 5.187 | 12.984 | **No** |
| Second comment edit | 5.203 | 13.297 | **No** |
| Dirty private mirror repaired | 3.063 | 3.829 | **No** |
| Ten-file comment edit | 16.125 | 20.969 | **No** |
| Add Python file | 5.109 | 11.422 | **No** |
| Remove added file | 11.812 | 11.860 | Yes |
| Rename first edited file | 4.719 | 12.000 | **No** |

On this fixture the repaired one-file scan loads the index without reconstructing the language-wide dependency graph. Nevertheless, its scoped adapter completes (~8 ms), the topology safety check reads only the selected prior file, then shared-object reconciliation reads the prior shared and touched file objects and detects a changed shared-object ID. The conservative safety path retries complete Python analysis (~6.5 seconds) on both one-file edits. This yields 12.984–13.297-second CLI walls and misses the provisional 10-second one-file target *even at package scale*. The measured repaired full adapter stage is 6.122 seconds; the additional requirement of one edit taking no more than 20% of that stage is also not met on this fixture. These package failures cannot substitute for the unmeasured full-Hermes warm target.

The old package incremental export contains 11,010 records following each comment-only edit; the repaired full-retry export contains 11,011. All initial full exports match exactly. Six of nine paired scenario fact digests differ. A separate exact-fact oracle has since adjudicated the **first one-file edit** (below); the remaining changed-scenario differences still require independent evaluation. Do not weaken the safety fallback to reproduce the old result.

## Independent one-file semantic oracle (same pinned package)

A fresh diagnostic ran three **independent, disposable** checkouts of the exact pinned `acp_adapter/` package. Each applied the identical appended `# phase5 edit_once` comment to `acp_adapter/__init__.py`. The reference performed a **fresh complete analysis of that edited source**, rather than treating either incremental result as ground truth.

| Analysis of the same edited source | Fact records | Relative to independent full reference |
| --- | ---: | --- |
| Fresh full analysis using repaired binary | 11,011 | Exact reference |
| Old incremental path | 11,010 | Missing exactly one shared edge; no extras |
| Repaired incremental path, including safety-triggered full retry | 11,011 | **Exact match** |

The missing old edge is `depends-on` with attribute `source: acp_adapter.tools`, `category: local`, and `path: true`. It was a cross-module dependency relationship, not a duplicate or harmless formatting difference. The old incremental result silently lost it on this comment-only edit; the repaired path preserves it but currently spends approximately 5–6 seconds rerunning full Python analysis. Its measured 9.5-second repeat in this separate diagnostic should not be substituted for the original 12.984-second acceptance measurement: the runs were not paired under identical external workload.

This establishes correctness of the repaired **first edit** but not successful Phase 5 performance acceptance. The remaining five changed-scenario hash mismatches have not been adjudicated against separate edited-source full references. An optimized incremental shared-fact policy must prove that it retains existing edges from untouched sources to stable changed-node identities, or take the conservative full fallback when source/target facts genuinely change. In particular, do not delete a shared edge merely because its target node belongs to an edited file if the scoped adapter cannot independently regenerate that cross-file edge.

Reproduction: `lexicon/evaluation/performance/incremental_phase5_shared_oracle.py`; result: `incremental-phase5-shared-oracle-2026-10-01.json`.

Package memory consumption improved despite the latency failures: first-edit sampled RSS was 137,003,008 bytes before and 41,955,328 bytes after. Memory alone does not satisfy the performance or semantic gates.

## Failure and recovery

Both package-level interruption probes terminated the scan subprocess, then completed a recovery scan and a second unchanged scan with a stable snapshot ID. The dedicated scan-engine test additionally forces adapter fingerprint drift *without* a source change and verifies a complete adapter rerun and a fresh indexed snapshot. These checks do not establish cancellation behaviour for a successfully initialized full Hermes snapshot.

## Acceptance gate disposition

| Gate | Evidence | Status |
| --- | --- | --- |
| Full Hermes cold publication | Timed out before Python extraction completed | **Blocked** |
| Full Hermes no-change within 5 s | No published full snapshot | Unmeasured |
| Full Hermes one-file within 10 s and 20% full-adapter time | No full warm run or completed full-adapter measurement | Unmeasured |
| Full Hermes ten-file baseline-derived bound | Neither full run reached the ten-file case | Unmeasured |
| No unrelated source/fact-object reads | Phase 4 synthetic gates pass; package dependency lookup bounded | Limited to fixtures |
| Full before/after semantic parity | Six package-level exported fact mismatches; first one-edit mismatch independently traced to an old missing cross-module edge; five others unadjudicated; no full run | **Failed diagnostic** |
| Peak RSS | Sampled; complete-run memory cap was not reached | Partial |
| Failure/interrupt lock release | Both package-level recoveries and adapter-drift integration pass | Passed at fixture level |

**Phase 5 cannot be closed and there is no operational cutover based on this report.** Do not change the current Arcana consumer's previously published snapshot or claim the full production latency targets.

## Follow-up repair and retest order

1. Profile the native Python cold-analysis path on the pinned full Hermes corpus after adapter discovery, with bounded per-shard progress and a defensible time/RSS limit. This is separate from the already repaired warm dependency planner.
2. The one-edit exact-fact oracle identifies an old missing `acp_adapter.tools` dependency edge and confirms that the repaired full retry produces the correct reference facts. Repair shared-fact invalidation so untouched cross-file dependencies are preserved when provably safe, and retain full retry for unprovable transitions. Adjudicate the other five mismatched mutation scenarios independently. Do not weaken ownership correctness.
3. Repeat the exact full pinned Hermes before/after sequence *after* the blockers are addressed. Require completed full-adapter time, zero unrelated warm reads, full semantic-fact identity (or adjudicated differences), ten-file bound, adapter drift, and success/failure/cancellation recovery before changing acceptance status.

## Artifacts and verification

- Runner: `lexicon/evaluation/performance/incremental_phase5_run.py` and its fixture/support module; explicitly caps cold/step time and sampled RSS.
- Comparator: `incremental_phase5_compare.py` and its five unit tests. Missing/censored data is never treated as a pass.
- Full cold failure: `incremental-phase5-hermes-full-after-2026-10-01.json`.
- Censored intermediate cold run: `incremental-phase5-hermes320-before-2026-10-01.json`.
- Independent one-edit fact oracle: `incremental-phase5-shared-oracle-2026-10-01.json`; reproducible `incremental_phase5_shared_oracle.py`. It confirms one old missing cross-module dependency and exact repaired/reference parity for that one edit.
- Paired package runs: `incremental-phase5-acp-before-2026-10-01.json`, `incremental-phase5-acp-after-2026-10-01.json`; result: `incremental-phase5-acp-comparison-2026-10-01.json`.
- Rust engine regression: `lexicon/tests/phase5_adapter_drift.rs`.
- Earlier [Phase 4 bounded-work evidence](lexicon-incremental-phase4-2026-10-01.md) remains valid for the independent 61- and 1,001-file synthetic checks.

## Related docs

- [Incremental scan performance repair plan](../planning/lexicon-incremental-scan-performance.md)
- [Phase 4 bounded-work report](lexicon-incremental-phase4-2026-10-01.md)
- [Testing and benchmark guidance](testing-and-benchmarks.md)
- [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md)

## Notes

The observed package performance regression is a conservative shared-fact full retry, not evidence that the new immutable dependency-index query loads unrelated fact objects. The 320-file derivative and the 14-file package are intentionally *not* described as full-Hermes acceptance. Every numeric finding is tied to the raw JSON; export digests identify a discrepancy but do not constitute an independently adjudicated semantic gold standard.