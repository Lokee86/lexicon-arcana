# Development

Development documentation defines current L+A verification/release practice and preserves measured historical evidence.

## Current verification and release

- [Documentation coverage](documentation-coverage.md) — active component, contract, stateful-flow, release, and benchmark owners.
- [Architecture verification](architecture-verification.md) — Pitlord and focused component/workflow gates.
- [Behavioral contract matrix](behavioral-contract-matrix.md) — active L+A invariants and protecting tests.
- [Release workflow](release-workflow.md) — root L+A build, install, packaging, protocol verification, and release artifacts.
- [Testing and benchmarks](testing-and-benchmarks.md) — component/evaluation procedures and retained report artifacts.
- [Arcana Lexicon ingestion heap baseline](arcana-lexicon-ingestion-heap-baseline-2026-09-28.md) — frozen pre-refactor heap, allocation, fixture, and artifact-oracle measurements for the bounded-memory ingestion refactor.
- [Arcana Lexicon ingestion final memory gate](../../arcana/evaluation/results/hermes-arcana-final-gate-2026-09-28/report.md) — bounded-memory refactor evidence for the frozen fixture; rerun verification after integration.
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
