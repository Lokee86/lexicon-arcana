# Lexicon Phase 5 follow-up — Shared facts and Python source locations

Parent index: [Development Documentation](INDEX.md)
Date: 2026-10-01
Status: Two verified repairs; **complete pinned-Hermes Phase 5 acceptance is still OPEN**.

## Purpose

Address the concrete regressions found in the original Phase 5 attempt without weakening semantic completeness or mistaking smaller pinned-package results for production release acceptance.

## Overview

Both repairs were implemented on `perf/lexicon-incremental-scan` against the pinned Hermes source commit `7798df0a83721df7ff44b9ba023d56b85b351d1e`. Benchmarks use disposable Git archives, a genuine Lexicon Rust CLI, sampled parent-plus-child RSS and bounded timeouts; the active Hermes worktree is untouched.

### Repair A: shared-fact delta preservation

The independent one-edit full-reference oracle found that the old incremental path silently lost one `acp_adapter.tools` `depends-on` cross-module edge. The Phase 3 conservative safety path preserved that record by performing complete Python analysis on each comment edit, but cost 12–13 seconds for one changed file in the initial 14-file package benchmark.

`materialize_merge.rs` now compares old and new touched-node semantic identities, retaining shared incoming edges from untouched files only when their referenced targets are stable. For a file node, changed source-content hashes alone do not invalidate its stable relationships; all other node metadata must agree. Removed/changed nodes remain conservatively unsafe and can force complete analysis. More importantly, shared merge keys now distinguish edge attributes/ownership and unresolved candidates; previously these records could collapse when endpoint/expression fields matched. A red/green storage regression has two same-endpoint, differently attributed dependency edges plus two distinct unresolved candidates.

The complete pinned `acp_adapter/` fixture (14 Python files) after the identity repair completed all nine mutation scenarios plus interruption/recovery. **All nine exported fact digests match the previous corrected full-retry output** for the corresponding source state. The first comment edit also matches the independently obtained full-source oracle, not merely the previous repaired binary.

| Pinned package CLI operation | Prior repaired run | Identity-repaired run |
| --- | ---: | ---: |
| Cold initialization | 12.484 s | 6.250 s |
| Unchanged | 2.672 s | 1.360 s |
| First comment edit | 12.984 s | **2.140 s** |
| Second comment edit | 13.297 s | **2.265 s** |
| Ten comment edits | 20.969 s | **5.734 s** |

The two one-file comment edits and ten-file comment edit no longer retry the full adapter; file add/remove/rename still take safe full analysis where required. Both runs are separate development-profile samples, **not** a controlled full-Hermes speedup calculation. The pinned package's separate 20%-of-full-adapter threshold is not proven by these wall times. Raw evidence: `incremental-phase5-acp-identity-fixed-2026-10-01.json`; original corrected reference: `incremental-phase5-acp-after-2026-10-01.json`.

### Repair B: indexed Python source positions

Opt-in `python.extract_shard_start`, `python.extract_large_file_start`, `python.extract_slow_file`, `python.extract_shard` and `python.extract_reduce` metrics isolated fact extraction after the native parser. Previous `source.rs` functions counted all newline bytes before an AST position **for every span and byte location**, causing approximately quadratic work in large Python files.

Each parsed `SourceFile` now owns a compact line-start offset index created once on load. Binary search determines its line; only the source line's UTF-8 characters are scanned to preserve exact existing code-point columns. Byte-offset columns and newline/CRLF semantics are unchanged. The line index's allocated size participates in extraction's retained-memory measurements. Parallel workers write complete metric lines atomically to stderr, preventing interleaved profile records.

The exact pinned Hermes `agent/` source subset (302 Python files, 6,302,285 source bytes) was run twice with an **85-second** cold cap, without changing the source revision:

| Measurement | Before line index | After line index |
| --- | ---: | ---: |
| Cold Lexicon CLI | **Timed out** at 85.359 s; no snapshot | **39.188 s**; published |
| Python extraction stage | Never completed | **5.777 s** |
| `auxiliary_client.py` visitor extraction | 70.143 s | **0.294 s** |
| `context_compressor.py` visitor extraction | 40.427 s | **0.213 s** |
| Extraction shards complete | 7 of 8 | **8 of 8** |
| Peak sampled parent + child RSS | 295,129,088 B | 395,960,320 B |

The completed run exported **249,297 Python facts**. A censored baseline cannot support a numeric overall speedup factor. Additional semantic verification: **98 Rust library unit tests, 18 Python adapter tests, and five focused scan-engine/shared-fact/delta tests passed**. The line-index unit test compares every UTF-8 character offset against the old location algorithm on Unicode, emoji, CRLF, blank lines and trailing newline examples.

Raw profiles: `incremental-phase5-agent-cold-profile-2026-10-01.json` and `incremental-phase5-agent-cold-indexed-2026-10-01.json`.

## Additional independent validation after line-index instrumentation

A later bounded production CLI run recompiled shared-merge repair and source-line indexing, then repeated fresh disposable pinned `acp_adapter/` initialization and two consecutive comment edits. It completed full initialization in **4.219 s**, unchanged scanning in **1.344 s**, and the two incremental edits in **2.547 s** and **2.687 s**, without any full-analysis retry. Each dependency query decoded zero fact objects; shared reconciliation read two objects.

A separate independent full analysis was performed **for each exact edited source state** using the same repaired binary. Both incremental exports match their corresponding independent full exports exactly by canonical fact hash and count (11,011 facts each). This extends the first-edit oracle to the second consecutive edit. The complete nine-scenario fixture gate uses the earlier identity-repaired run; the other changed-source cases remain to be independently adjudicated.

Evidence: `incremental-phase5-shared-repair-package-2026-10-01.json`, `incremental-phase5-shared-repair-oracle-2026-10-01.json`. Reproduction: `incremental_phase5_current_oracle.py`.

## Remaining production acceptance gaps

- Complete pinned Hermes contains **7,114 Python files / 88,964,777 source bytes**. The fixed 302-file `agent/` subset is only 6.3 MB. No completed identical before/after full-Hermes run exists; the original repaired full attempt timed out at 240 seconds before extraction finished.
- With extraction repaired, the measured 302-file cold profile exposes other owners: full Python adapter ~12.94 s (including extraction and resolution), immutable dependency-index construction **7.68 s**, full language materialization **10.65 s** (includes the index build) and interstack contract detection **9.40 s** (interstack total **10.29 s**). These are overlapping nested stages; do not add nested timers together. Their cost at 7,114 files is unmeasured.
- Pinned 14-file package's remaining mutation scenarios match the **previous corrected full-retry facts**, but independent full-source reference runs for every changed source state are still required for conclusive full semantic parity. Other language adapters and a complete-source cancellation case remain outside this fixture.
- The Phase 5 criteria still require full pinned Hermes complete cold publication, no-change ≤5 s, one-file ≤10 s **and** ≤20% of completed full-adapter time, a completed prior ten-file bound, zero unrelated warm fact/source reads, memory envelope and interrupted-run recovery. Preserve old and new raw evidence for audit; do not mark Phase 5 passed without these gates.

## Verification

`lexicon/evaluation/performance/incremental_phase5_repair_gate.py` validates exact pinned-source fixture identity, all nine package export digests, package warm bounds, absence of unnecessary full retries, stable package recovery, and bounded agent cold completion. `test_incremental_phase5_repair_gate.py` mutates facts, completeness, provenance, retry, and recovery evidence and requires failure. **This is explicitly a repair-fixture gate, not the release gate.**

Production changes are limited to Python source-location indexing/profiling and stable shared-fact reconciliation. Immutable dependency planning, publication ownership, and Arcana consumption remain unchanged.

## Related docs

- [Original blocked Phase 5 acceptance and independent one-edit oracle](lexicon-incremental-phase5-2026-10-01.md)
- [Phase 4 bounded dependency-planner proof](lexicon-incremental-phase4-2026-10-01.md)
- [Performance hard-cut repair plan](../planning/lexicon-incremental-scan-performance.md)
- [Development tests and benchmarks](testing-and-benchmarks.md)

## Notes

Do not rerun the full pinned Hermes workload until the now-visible dependency-index materialization and interstack contract-detection costs have been assessed under an explicit wall/RSS budget. A 302-file completed subset does not establish 7,114-file production latency, even when the previous quadratic extraction path is fixed.