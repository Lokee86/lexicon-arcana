# C-family Phase 2.7 restart handoff

Parent index: [Development Documentation](INDEX.md)

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

The focused C-family unit surface is now **23/23** green. The generic frontend runner is **5/5** green, including preservation of first-frame semantics on the new spooled response path.

## Current Git gate

Git's real build-context gate exposed a protocol-capacity issue rather than a semantic failure. The helper response exceeds the previous 128 MiB and 512 MiB in-memory response ceilings.

The frontend path has therefore been changed to:

- spool a bounded response frame to a temporary file;
- preserve the existing first newline-delimited JSON frame contract;
- wait for the helper to exit and release Clang AST/Sema memory before Rust JSON decode/materialization;
- retain a hard **1 GiB** response ceiling;
- bound helper-process lifetime by sending **source files only** in deterministic **8-file** batches. Each source helper exits before the next batch, releasing all Clang state. Rust writes returned per-file observation fragments to a temporary JSONL spool and merges/materializes them file-by-file, so batched responses do not simply accumulate in Rust memory. Still-unobserved headers are then sent together in one helper request so the native helper retains ownership of its existing shallowest-orphan-header policy.

The latest real-build-context Git attempt reached Clang frontend execution but the single helper exited before emitting a response frame. Codebase Memory reproduced the failure on the first **128-file** source request and again at **16 files**. The 16-file attempt was confirmed by the WSL OOM killer: the helper reached about **6.55 GB anonymous RSS**. These are helper-lifetime memory failures, not semantic mismatches and not response-ceiling failures. The production source batch is therefore **8 files**, giving each of the default eight native workers one source TU before process teardown.

The replacement production path sends source files through deterministic **8-file helper requests** and spools returned per-file observations to disk. The spool-backed nlohmann/json canary completed deterministically with the same semantic counts and hash at **37.853 s / 34.710 s** cold/warm wall. Codebase Memory is the current large-source batching gate because its 623-file frozen corpus crosses the new helper boundary while remaining smaller than Git.

Git/CBM final runs use raw WSL Docker rather than the Workspace Docker scheduler because the latter repeatedly cancelled long jobs when unrelated Docker work took the shared target slot.

Exact final-gate command shape:

`scripts/c_family_phase2_final_case.sh git /corpus/git`

using the pinned Clang 18 image, clean helper, eight native frontend workers per helper process, and `C:\!bin\tmp\phase27\adapter_eval`.

Do not treat earlier scheduler cancellations as corpus failures.

## Resumable final-gate harness

Large final gates no longer need to complete cold and warm passes in one process. `scripts/c_family_phase2_final_case.sh` accepts an optional third mode:

- `cold`: run only the cold pass and immediately persist its timing plus fact summary into `/out/final-matrix.json`;
- `warm`: rerun only the warm pass and verify its SHA-256 against the stored cold fact hash;
- omitted / `full`: preserve the original cold+warm behaviour.

The Python runner also aggregates repeated `[lexicon-perf]` stages across multiple helper invocations so batched-source timing evidence reflects total frontend work rather than only the final helper process.

## Remaining Phase 2.7 work

The previous plan to finish Codebase Memory and then Git directly is superseded by Phase 2.7R. Codebase Memory exceeded the 600-second gate while incomplete after 200 translation units, with about 489 MB of helper response data, about 6.9 GB peak process-tree RSS, about 307.5 s of frontend work, and about 167.1 s of Rust response decoding.

The repair sequence must land before calibration resumes. Do not run Git or any larger final corpus gate until Codebase Memory passes the repaired ownership architecture. Phase 2.7 remains blocked until Phase 2.7R is complete, the corpus matrix is rerun in increasing size, and final semantic/performance adjudication is recorded.

## Environment notes

Native calibration requires the WSL Docker engine and pinned `lexicon-cfamily-phase27:clang18` image. The host Windows installation does not provide LLVM/Clang development packages.

For long calibration runs, invoke Docker through raw `wsl.exe docker ...` from outside the repo worktree. The structured Workspace Docker target has a shared scheduler slot and repeatedly cancelled valid long-running corpus jobs when unrelated Docker jobs started.

Keep large build products, response spools, and fact streams Linux/container-local where practical. Persist compact summaries/evidence to the Windows side.

Do not spend turns polling long Clang jobs. The calibration runner retains the 600-second per-run performance cap.

## Checkpoint policy

This restart checkpoint intentionally preserves work in progress. Phase 2.7 remains open until the remaining pinned-corpus final gates and final verification are recorded.
