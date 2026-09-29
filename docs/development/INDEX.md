# Development

Development documentation defines current L+A verification/release practice and preserves measured historical evidence.

## Current verification and release

- [Documentation coverage](documentation-coverage.md) — active component, contract, stateful-flow, release, and benchmark owners.
- [Architecture verification](architecture-verification.md) — Pitlord and focused component/workflow gates.
- [Behavioral contract matrix](behavioral-contract-matrix.md) — active L+A invariants and protecting tests.
- [Release workflow](release-workflow.md) — root L+A build, install, packaging, protocol verification, and release artifacts.
- [Testing and benchmarks](testing-and-benchmarks.md) — component/evaluation procedures and retained report artifacts.
- [Arcana Lexicon ingestion heap baseline](arcana-lexicon-ingestion-heap-baseline-2026-09-28.md) — frozen pre-refactor heap, allocation, fixture, and artifact-oracle measurements for the bounded-memory ingestion refactor.
- [C-family Phase 2 oracle freeze](c-family-phase2-oracle-freeze-2026-09-28.md) — pre-Clang canonical fixture, pinned C/C++ semantic metrics, wall/RSS baselines, and the Phase 2 calibration boundary.
- [C-family Phase 2.2 Clang runtime boundary](c-family-phase2-clang-boundary-2026-09-28.md) — private LibTooling protocol/runtime seam, diagnostics, packaging ownership, and pre-cutover verification.
- [C-family Phase 2.3 structural Clang observations](c-family-phase2-structural-clang-2026-09-28.md) — staged Clang translation-unit/declaration/include/macro evidence, Rust identity materialization, and pre-production structural parity.
- [C-family Phase 2.4 semantic Clang observations](c-family-phase2-semantic-clang-2026-09-28.md) — staged Clang inheritance/override/call/receiver/overload evidence with Rust-owned target, certainty, and unresolved policy.
- [C-family Phase 2.5 Clang value-flow rebase](c-family-phase2-value-flow-clang-2026-09-29.md) — compiler-bound arguments, pointer/callback flow, reads/writes, passes-to, and macro-expansion provenance with Rust-owned graph policy.
- [C-family Phase 2.6 production Clang cutover](c-family-phase2-production-cutover-2026-09-29.md) — hard cut to the sole Clang production frontend, deletion of C/C++ Tree-sitter/compiler reconstruction, and final Rust/Clang ownership boundary.
- [Lexicon-wide performance restoration](lexicon-wide-performance-restoration.md) — active Rust-wide restoration project; Phases 0, 1, 2, 4, 5, 6, 7, and 8 complete, with Phase 3 still pending.
- [Lexicon-wide pre-port optimization parity matrix](lexicon-wide-optimization-parity-matrix.md) — permanent Phase 5 audit of the mature pre-Rust performance sequence and current ownership/status.
- [Lexicon Go-path performance restoration](lexicon-performance-restoration.md) — completed Go adapter/helper optimization project retained as historical evidence.
- [Lexicon optimization parity audit](lexicon-optimization-parity-audit.md) — Go-path Phase 8 comparison with the mature Go optimization oracle.
- [Go adapter Phase 19 final cleanup](go-adapter-phase19-final-cleanup-2026-09-27.md) — completed hard cut to one Rust/private-helper production path with frozen compatibility goldens.
- [Go adapter Phase 18 packaging/runtime](go-adapter-phase18-packaging-runtime-2026-09-27.md) — packaged private helper, deterministic discovery/versioning, runtime diagnostics, and installed-tree acceptance evidence.
- [Go adapter Phase 17 cutover](go-adapter-phase17-cutover-2026-09-27.md) — native Rust production ownership, retired legacy runtime path, and full/incremental cutover evidence.
- [Go adapter Phase 16 calibration](go-adapter-phase16-calibration-2026-09-27.md) — completed pinned real-repository, determinism, and Arcana acceptance evidence.
- [Go adapter Phase 16 freeze](go-adapter-port-freeze-2026-09-26.md) — historical checkpoint and resumption guardrails.

## Research and historical evidence

- [Agent benchmark findings](agent-benchmark-findings.md) — Grimoire historical comparisons and current L+A experiments.
- [Retrieval quality](retrieval-quality.md) — historical retrieval-pipeline methodology and result interpretation where still relevant to preserved experiments.
- [Ranking calibration corpus](ranking-calibration-corpus.md) — judged-case design retained as research evidence.
- [Recent changes — July 2026](recent-changes-2026-07.md) — historical Grimoire development record.

Checked-in Grimoire evaluation corpora and reports remain evidence for their recorded revisions/conditions. They are not active Grimoire product guarantees after [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md).

Future repository-agent comparisons normally use Plain versus Lexicon + Arcana; Grimoire conditions are retained as historical baselines only.
