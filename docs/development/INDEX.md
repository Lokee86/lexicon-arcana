# Development

Development documentation defines current L+A verification/release practice and preserves measured historical evidence.

## Current verification and release

- [Documentation coverage](documentation-coverage.md) — active component, contract, stateful-flow, release, and benchmark owners.
- [Architecture verification](architecture-verification.md) — Pitlord and focused component/workflow gates.
- [Behavioral contract matrix](behavioral-contract-matrix.md) — active L+A invariants and protecting tests.
- [Release workflow](release-workflow.md) — root L+A build, install, packaging, protocol verification, and release artifacts.
- [Testing and benchmarks](testing-and-benchmarks.md) — component/evaluation procedures and retained report artifacts.
- [Lexicon-wide performance restoration](lexicon-wide-performance-restoration.md) — active Rust-wide restoration project; Phase 0 lifecycle profile and Hermes pathological baseline.
- [Lexicon Go-path performance restoration](lexicon-performance-restoration.md) — completed Go adapter/helper optimization project retained as historical evidence.
- [Lexicon optimization parity audit](lexicon-optimization-parity-audit.md) — Go-path Phase 8 comparison with the mature Go optimization oracle.
- [Go adapter Phase 16 freeze](go-adapter-port-freeze-2026-09-26.md) — paused migration checkpoint and resumption guardrails.

## Research and historical evidence

- [Agent benchmark findings](agent-benchmark-findings.md) — Grimoire historical comparisons and current L+A experiments.
- [Retrieval quality](retrieval-quality.md) — historical retrieval-pipeline methodology and result interpretation where still relevant to preserved experiments.
- [Ranking calibration corpus](ranking-calibration-corpus.md) — judged-case design retained as research evidence.
- [Recent changes — July 2026](recent-changes-2026-07.md) — historical Grimoire development record.

Checked-in Grimoire evaluation corpora and reports remain evidence for their recorded revisions/conditions. They are not active Grimoire product guarantees after [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md).

Future repository-agent comparisons normally use Plain versus Lexicon + Arcana; Grimoire conditions are retained as historical baselines only.
