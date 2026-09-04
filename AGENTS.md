# AGENTS.md

This repository is the Lexicon + Arcana product family. Grimoire's former discovery runtime, MCP surface, repository state, and installed skill are retired under ADR 0006.

## Read first

- `README.md`
- `docs/INDEX.md`
- `docs/architecture/system-overview.md`
- `docs/architecture/components.md`
- `docs/architecture/analysis-stack.md`
- `docs/documentation-policy.md`
- `docs/documentation-procedure.md`
- `docs/development/documentation-coverage.md`
- `docs/development/behavioral-contract-matrix.md`

For component work:

- `lexicon/docs/README.md`
- `arcana/docs/README.md`

## Ownership rules

- Lexicon owns language adapters, normalized semantic facts, immutable analysis objects, snapshots, and publication lifecycle.
- Arcana owns verified Lexicon ingestion, repository/call graph storage, traversal, impact, paths, overlays, compaction, and `arcana.query.v1`.
- Warlock or another consumer owns probabilistic task/context orchestration.
- Ordinary source reads, search, IDE, and Git remain first-class evidence surfaces.
- Do not recreate Grimoire's retired umbrella discovery, MCP, stable-handle, session, document-index, or repository-state layer inside Lexicon or Arcana.
- Keep component behavior in its owning component and preserve deterministic publication, ordering, and explicit degradation behavior.
- Exclude generated targets, caches, tool state, and worktrees from repository-wide scans.

## Historical material

Historical Grimoire ADRs, benchmark results, reports, and reference pages may retain retired names and behavior when needed to explain prior evidence. They are not current product contracts. Current implementation claims must resolve to Lexicon, Arcana, shared release tooling, or an explicit downstream consumer.

## Documentation discipline

Documentation is part of the implementation. Update the owning component or shared product documentation in the same change. Update `docs/development/documentation-coverage.md` when a command, component, stateful flow, release surface, or machine-readable contract changes. Update `docs/development/behavioral-contract-matrix.md` when a durable invariant or protecting test changes. Keep current behavior separate from planning, research, historical records, and limitations.

Do not report documentation as complete or current unless the configured shared, Lexicon, and Arcana checks pass and known gaps are disclosed.

## Completion report

```text
Documentation impact:
- Inspected:
- Updated:
- Not affected:
- Compliance check:
- Known documentation gaps:
```
