# Go Adapter Port Freeze — Phase 16

Parent index: [Development Documentation](INDEX.md)

**Frozen:** 2026-09-26  
**Branch:** `feature/lexicon-rust-library`  
**Code checkpoint:** `c8dce3fee4951601ce414065061d2757f891a69c` — `Calibrate native Go adapter on real repository`

## Purpose

Freeze the native Rust Go-adapter port so the migration can be resumed without reconstructing intent from chat history.

## Overview

Phases 0–15 were complete when this checkpoint was frozen. The separate performance-restoration project subsequently completed, Phase 16 calibration resumed and completed, and Phase 17 cutover is now complete. Packaging/runtime and final cleanup remain unstarted. See [Go Adapter Phase 16 Calibration — 2026-09-27](go-adapter-phase16-calibration-2026-09-27.md) and [Go Adapter Phase 17 Cutover — 2026-09-27](go-adapter-phase17-cutover-2026-09-27.md) for completion evidence.

## Migration status

- **Phases 0–15:** complete.
- **Phase 16 — real-repository calibration:** **COMPLETE — 2026-09-27.**
- **Phase 17 — cutover:** **COMPLETE — 2026-09-27.**
- **Phase 18 — packaging/runtime:** not started.
- **Phase 19 — docs/cleanup:** not started.

Do **not** treat `c8dce3f` alone as completion of Phase 16. It is the durable freeze checkpoint; final completion evidence is recorded in [Go Adapter Phase 16 Calibration — 2026-09-27](go-adapter-phase16-calibration-2026-09-27.md).

## Recent phase checkpoints

| Phase | Commit | State |
| --- | --- | --- |
| 12 — Dependency graph ownership | `ebbcfd6` | Complete |
| 13 — Incremental ownership | `16f8769` | Complete |
| 14 — Deterministic semantic parallelism | `bf18624` | Complete |
| 15 — Differential parity harness | `cac4aad` | Complete |
| 16 — Real-repository calibration | `c8dce3f` checkpoint + 2026-09-27 completion change | **Complete** |
| 17 — Cutover | 2026-09-27 cutover change | **Complete** |

## What Phase 16 is supposed to prove

Calibrate the native Rust + private Go semantic-helper path against representative retained repositories and the legacy Go adapter.

The intended cases are:

1. **Demon Docs** at `fa5ca9aea12e20c29c378d5d018647958b862cac`.
2. **Space Rocks** at `431625042dbdb1a884954cab6ec726413aa36e2b`.
3. **Lexicon** as the larger self-hosting case.
4. Arcana ingestion/query behaviour over the resulting facts.

The gate is not merely similar record counts. The migration requires deterministic output with no unexplained semantic regression.

## What the Demon Docs calibration found

The first real-repository pass exposed behaviour that the permanent fixture suite did not exercise. The Phase 16 checkpoint contains fixes for:

- private-helper response frames exceeding the old 16 MiB cap;
- package-level function literals that must remain synthetic SSA functions rather than structural closures;
- duplicate package/test views of the same callsite, including resolved-over-unresolved precedence;
- external interface method contracts;
- anonymous internal interface method contracts with source provenance;
- the predeclared `error` interface using the legacy `go:unknown` namespace;
- per-module SSA/VTA analysis rather than collapsing module roots together;
- preserving direct interface contracts when VTA contributes concrete targets;
- internal interface implementation expansion;
- capture-only variable span behaviour;
- source-derived edge ownership used by the legacy facts encoder;
- generated `*.test` package `main` functions;
- generated/synthetic function materialization needed by real repositories.

The checkpoint also contains:

- `evaluation/go_adapter_calibration.py` for streaming semantic comparison;
- `lexicon/examples/go_adapter_snapshot.rs` for producing canonical native Go facts;
- focused regressions for the real-repository cases above.

## Last verified state before the freeze

The durable regression state reached during Phase 16 was:

- Go semantic-helper tests: green.
- Native Rust Go-adapter suite: **30/30 green**.
- Phase 15 live legacy/native fixture differential gate: green.
- The nested generated-test-main helper regression: green.

For Demon Docs, the calibration was reduced from hundreds of semantic differences to a state where:

- edge/relation totals matched legacy;
- unresolved calls were eliminated;
- the remaining observed drift was narrowed to generated test-main side-effect nodes.

A final real-repository comparison after the latest generated-test-main fix was **not durably verified** before the temporary calibration artifacts were cleaned up. Re-run that comparison when Phase 16 resumes. Do not infer final Demon Docs parity from the checkpoint alone.

## Performance issue — first task on resume

**Do not continue brute-force Phase 16 scans yet.**

The native Demon Docs calibration scans were taking roughly 40–50 seconds even with the `4 workers / 8 shards / fan-in 4` execution plan.

The port has not yet preserved all of the optimization work from the legacy Go adapter. That optimization gap must be audited and restored before using the current native scan time as a calibration baseline.

The first resumption task is therefore:

1. compare the legacy Go adapter's performance machinery with the native Rust + helper path;
2. identify optimization work that was skipped during the semantic extraction/port;
3. restore those optimizations without changing the frozen semantic contract;
4. gate optimization changes against the Phase 15 differential harness and the Phase 16 focused regressions;
5. only then resume repeated real-repository calibration.

Performance work must not weaken parity checks or silently alter the semantic contract.

The independent restoration work is tracked in [Lexicon Performance Restoration](lexicon-performance-restoration.md). The Go adapter migration remains frozen at Phase 16 until that work is complete.

## Phase 16 resumption order

After restoring the omitted optimization work:

1. Run the complete Go helper and native Go adapter suites.
2. Run the Phase 15 differential fixture matrix.
3. Rebuild the current legacy Go oracle from source.
4. Re-run Demon Docs once at the pinned revision.
5. Compare complete canonical facts with the calibration comparator.
6. Repeat the native run to verify determinism.
7. Calibrate Space Rocks at its pinned revision.
8. Calibrate Lexicon as the self-hosting repository.
9. Run the intended Arcana ingestion/query checks.
10. Record the calibration results in durable repository documentation.
11. Only then mark Phase 16 complete.

Temporary multi-megabyte calibration outputs should remain disposable; preserve compact result summaries and regressions in the repository instead.

## Phase 17 — Cutover

**Complete — 2026-09-27.** See [Go Adapter Phase 17 Cutover](go-adapter-phase17-cutover-2026-09-27.md).

- the native Rust `GoAdapter` is the authoritative Go adapter;
- the legacy Go application runner refuses the standalone Go facts-v1 adapter runtime path;
- the root build workflow no longer builds `lexicon-go` as a production adapter;
- the private Go semantic helper remains the deliberate language-native helper boundary;
- full and incremental scans are protected by a native `ScanEngine` cutover regression;
- the standalone legacy adapter remains oracle/history only until cleanup.

**End condition satisfied:** the legacy Go adapter is no longer a runtime fallback or production path.

## Phase 18 — Packaging/runtime

Make the native adapter + private semantic helper deployable outside the development checkout:

- helper discovery;
- executable naming;
- build/install layout;
- helper/adapter fingerprinting and version behaviour;
- platform/runtime handling;
- useful failure diagnostics;
- packaged-runtime tests.

**End condition:** users do not need a Go development environment or repository-local build artifacts to use the Go adapter.

## Phase 19 — Docs/cleanup

Once cutover and packaging are proven:

- remove the retired legacy runtime adapter;
- remove migration-only compatibility scaffolding that is no longer useful;
- retain parity/calibration tests that continue to protect behaviour where appropriate;
- remove stale docs and dead code;
- update adapter-development documentation and the behavioural contract matrix;
- document the final Rust/helper ownership boundary.

## Guardrails preserved from resumption

- Phase 17 must build on the completed Phase 16 real-repository evidence, not fixture parity alone.
- Preserve the Phase 16 calibration report and focused regressions through cutover.
- Do not redesign semantics during calibration or optimization.
- Do not weaken the Phase 15 differential comparator to make a calibration pass.
- Keep the legacy implementation available as oracle/history, but do not restore it as a runtime fallback.
- Preserve deterministic output across worker/shard/fan-in configurations.
- Treat the current performance regression as unfinished port work, not an acceptable new baseline.

## Related docs

- [Lexicon performance restoration](lexicon-performance-restoration.md)
- [Testing and benchmarks](testing-and-benchmarks.md)
- [Lexicon Rust migration](../../lexicon/docs/RUST_MIGRATION.md)

## Notes

This is a dated migration checkpoint. It records the state needed to resume Phase 16; it does not redefine the current Go adapter semantic contract or mark Phase 16 complete.
