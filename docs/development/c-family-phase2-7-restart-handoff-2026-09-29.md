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

The durable calibration report now contains final deterministic cold/warm evidence for:

- LevelDB;
- fmt;
- Catch2;
- nlohmann/json.

Catch2 calibration exposed and closed two additional correctness defects:

- external compiler-observed function-pointer fields such as `sigaction.sa_handler` are not repository-owned pointer nodes;
- duplicate header call observations must merge contextual metadata deterministically rather than use arrival-order last-wins semantics.

The focused C-family unit surface is now **22/22** green. The generic frontend runner is **5/5** green, including preservation of first-frame semantics on the new spooled response path.

## Current Git gate

Git's real build-context gate exposed a protocol-capacity issue rather than a semantic failure. The helper response exceeds the previous 128 MiB and 512 MiB in-memory response ceilings.

The frontend path has therefore been changed to:

- spool a bounded response frame to a temporary file;
- preserve the existing first newline-delimited JSON frame contract;
- wait for the helper to exit and release Clang AST/Sema memory before Rust JSON decode/materialization;
- retain a hard **1 GiB** response ceiling;
- bound helper-process lifetime by sending source files in deterministic **128-file** batches, merging exact duplicate observations in Rust, then analyzing only still-unobserved requested headers in shallowest-depth order. Each helper exits before the next batch, releasing all Clang state.

The latest real-build-context Git attempt reached Clang frontend execution but the single helper exited before emitting a response frame. That attempt is evidence of helper-lifetime memory pressure, not a semantic mismatch and not a response-ceiling failure.

The replacement production path now sends source files through deterministic **128-file helper requests** and merges responses in Rust. A short nlohmann/json canonical-hash canary is the immediate gate before retrying Git.

Git/CBM final runs use raw WSL Docker rather than the Workspace Docker scheduler because the latter repeatedly cancelled long jobs when unrelated Docker work took the shared target slot.

Exact final-gate command shape:

`scripts/c_family_phase2_final_case.sh git /corpus/git`

using the pinned Clang 18 image, clean helper, eight native frontend workers per helper process, and `C:\!bin\tmp\phase27\adapter_eval`.

Do not treat earlier scheduler cancellations as corpus failures.

## Remaining Phase 2.7 work

At this checkpoint only the two large final corpus gates remain:

1. finish Git `9a0c4701dcd5725c4184599322b52933ff5005ca`;
2. run Codebase Memory `97ce23f9827177fff3858831156e9795c6832b18`;
3. compare both against the Phase 2.1 oracle and append their semantic/performance adjudication;
4. run final formatting/diff/focused acceptance;
5. mark Phase 2.7 complete and only then proceed to Phase 2.8.

## Environment notes

Native calibration requires the WSL Docker engine and pinned `lexicon-cfamily-phase27:clang18` image. The host Windows installation does not provide LLVM/Clang development packages.

For long calibration runs, invoke Docker through raw `wsl.exe docker ...` from outside the repo worktree. The structured Workspace Docker target has a shared scheduler slot and repeatedly cancelled valid long-running corpus jobs when unrelated Docker jobs started.

Keep large build products, response spools, and fact streams Linux/container-local where practical. Persist compact summaries/evidence to the Windows side.

Do not spend turns polling long Clang jobs. The calibration runner retains the 600-second per-run performance cap.

## Checkpoint policy

This restart checkpoint intentionally preserves work in progress. Phase 2.7 remains open until the remaining pinned-corpus final gates and final verification are recorded.
