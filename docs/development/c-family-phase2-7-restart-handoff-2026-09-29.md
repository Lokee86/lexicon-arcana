# C-family Phase 2.7 restart handoff

**Branch:** `refactor/c-family-clang`

**Base:** `610f840` — `Cut C-family production over to Clang`

**Checkpoint time:** 2026-09-29

## Current state

Phase 2.7 is not complete, but the work is preserved in this checkpoint. The durable calibration report is:

- `docs/development/c-family-phase2-calibration-2026-09-29.md`

That report contains the current semantic adjudication, toolchain definition, staged performance evidence, completed final gates, and remaining matrix.

Important completed work in this checkpoint:

- pinned Clang/LLVM 18 calibration Docker image;
- Phase 2 calibration runner instrumentation and reusable fact metrics;
- baseline delta comparator;
- Make dry-run compilation-database generator;
- final per-corpus container runner;
- staged frontend/IPC/materialization/graph/canonicalization measurements;
- bounded parallel Clang translation-unit execution;
- Clang 18 build/API compatibility repairs;
- compilation-database path normalization;
- source-range robustness for macro-derived observations;
- template-instantiation compiler-ID normalization;
- lambda semantic-source attribution to the nearest source-written callable;
- token-pasted/macro-generated callable ownership fixes;
- explicit invariant that semantic call/access/value-flow sources have a materialized callable declaration;
- Rust materialization support for compiler-owned source identities across files;
- focused regressions and whole-system gates described in the calibration report.

## Completed corpus evidence

The report currently contains durable evidence for:

- LevelDB diagnostic calibration and stage profile;
- fmt final cold/warm gate with deterministic canonical facts;
- nlohmann/json semantic calibration and stage profile;
- Git synthetic/no-build-context timeout evidence.

The report also records the current non-corpus acceptance gates, including the focused C-family tests, doctor/scan workflow checks, helper packaging, relocation determinism, and multi-language baseline verification.

## Exact interrupted operation

A final Git corpus gate was running through:

`scripts/c_family_phase2_final_case.sh git /corpus/git`

using the pinned Clang 18 container, clean native helper, eight frontend workers, and a writable snapshot of the pinned Git checkout.

That job was explicitly cancelled for the user-requested restart. Do **not** treat it as a failed gate.

## Remaining Phase 2.7 work

Resume the final cold/warm matrix from the existing runner. The remaining cases listed in the calibration report are authoritative; at checkpoint time they include Git, Codebase Memory, LevelDB final deterministic rerun, Catch2, and nlohmann/json final deterministic rerun.

For each remaining case:

1. run the pinned corpus revision through `scripts/c_family_phase2_final_case.sh`;
2. preserve cold/warm wall time, process-tree RSS, stage metrics, and canonical SHA-256;
3. compare against the Phase 2.1 frozen oracle with the Phase 2 comparator;
4. adjudicate semantic deltas against the authoritative-Clang ownership rules rather than restoring Tree-sitter guesses;
5. append the evidence to the calibration report;
6. rerun focused/full acceptance checks;
7. only then mark Phase 2.7 complete and proceed to Phase 2.8 deletion work.

## Environment notes

Native calibration requires the WSL Docker target and the pinned `lexicon-cfamily-phase27:clang18` image. The host Windows installation does not provide LLVM/Clang development packages.

Transient WSL/Docker wrapper hangs were observed around Windows bind-mount handoff. The reliable pattern is to keep build products and fact streams on Linux/container-local storage and persist only compact summaries/evidence to the Windows worktree.

Do not spend turns polling long Clang jobs. Use the 600-second per-case cap as the performance result when a case reaches it.

## Checkpoint policy

This restart checkpoint intentionally preserves work in progress. Phase 2.7 remains open until the remaining pinned-corpus final gates and final verification are recorded.
