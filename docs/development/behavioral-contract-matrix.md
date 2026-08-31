# Behavioral Contract Matrix

Parent index: [Development Documentation](INDEX.md)

## Purpose

This document maps critical Grimoire-stack invariants to focused tests and release gates.

## Overview

The matrix covers contracts that cross retrieval lanes, snapshots, process boundaries, storage engines, or independently usable components.

## Contracts

| Contract | Primary verification owner |
| --- | --- |
| Source and documentation remain separately ranked evidence classes | Agent-query, agent-runtime, and knowledge retrieval tests; judged retrieval corpora |
| Narrow discovery returns bounded handle-first evidence and defers exact expansion to inspection | Agent-query and MCP contract tests; agent-discovery benchmark harness |
| Stable handles are snapshot-qualified and resolve exact evidence | Agent-query/session tests and end-to-end discovery fixtures |
| Repository preparation aligns Grimoire, Lexicon, Arcana, and document state before current-only follow-ups | Repository-state and application orchestration tests |
| Repository freshness covers every file class that can affect Grimoire retrieval or Lexicon output, including Lexicon-only language/config inputs, ignore controls, and analysis-relevant files above the source-index size cap | `internal/repostate/fingerprint_*_test.go` |
| Stale Lexicon or Arcana snapshots are never exposed as active discovery providers; healthy source/document lanes may still degrade independently | Agent-runtime provider-freshness tests, agent-query prepared-snapshot tests, and repository-state tests |
| Exact source search remains available independently of embeddings | Source retrieval tests and no-vector integration tests |
| Lexicon owns parsing and normalized language facts; Grimoire and Arcana do not duplicate adapters | Lexicon adapter tests, component-boundary docs, release workflow checks |
| Arcana consumes Lexicon facts and owns graph storage and traversal rather than language parsing | Arcana ingestion, protocol, snapshot, storage, and traversal tests |
| Immutable Lexicon and Arcana objects are content-addressed and safely reusable across repository state | Lexicon snapshot/object tests and Arcana snapshot/storage tests |
| Lexicon serializes writers, records pending publication before advancing state, and recovers or rejects incomplete transactions explicitly | Lexicon lock, transaction, pending-publication, object-store, and scanner tests |
| Arcana serializes managed-state writers and replaces `CURRENT` only after a complete verified generation is published | Arcana sync-state, repository-snapshot, graph-manifest, and publication-failure tests |
| Arcana overlays validate base identity and compact without changing graph meaning | Overlay and compaction tests |
| Vector features remain optional and do not make source discovery depend on repository-wide code embeddings | Grimoire knowledge-vector and Arcana vector tests; no-vector workflow tests |
| Result ordering, lane budgets, and assessment output are deterministic | Agent-query and ranking calibration tests |
| Session deltas apply one global lane-preserving hit budget before canonical evidence pruning | Agent-runtime investigation-budget tests |
| Impact merges duplicate Lexicon and Arcana dependents and ranks production-relevant, definite, shallow evidence for the current query | Agent-query impact-shaping tests |
| Release bundles preserve independently runnable `grimoire`, `lexicon`, and `arcana` components | Packaging and installation smoke tests |
| Installed release preparation preserves known C-family unresolved reasons and promotes unknown future reason labels through Arcana into Grimoire compatibility warnings | `internal/repostate/warnings_test.go`, `scripts/test_installed_mcp.py`, `scripts/installed_warning_contract.py` |
| Selecting Grimoire for installation also installs its required Lexicon/Arcana providers and Lexicon adapters | `scripts/test_workflow.py` installation smoke tests |
| Combined builds publish an Arcana binary that negotiates the required `arcana.query.v1` capability set before the bundle is accepted | Root workflow Arcana protocol verification and workflow smoke tests |
| Root, Lexicon, and Arcana documentation trees pass without baselines, and focused code maps remain with their canonical owners | Shared checker, `scripts/check_docs.py`, and documentation-standard CI |

## Release gates

```bash
python scripts/workflow.py test
python .standards/docs_policy/check.py --repo .
python .standards/docs_policy/check.py --repo . --config docs-standard.lexicon.json
python .standards/docs_policy/check.py --repo . --config docs-standard.arcana.json
python scripts/check_docs.py
```

## Code map

| Matrix concern | Primary implementation or artifact | Protecting tests/gates |
| --- | --- | --- |
| Discovery contracts | `internal/agentquery/`, `internal/agentruntime/` | package tests and agent benchmark corpus |
| Prepared-state publication | `internal/index/`, `internal/repostate/` | index/repostate tests and workflow test gate |
| Lexicon snapshot semantics | `lexicon/internal/scan/`, `lexicon/internal/objectstore/` | Lexicon complete test matrix |
| Arcana graph publication and traversal | `arcana/src/repository/`, `storage/`, `snapshot/`, `protocol/` | Arcana Cargo test suite |
| Documentation and change-impact contracts | `.standards/docs_policy/`, `scripts/check_docs.py` | documentation-standard workflow |
| Release packaging | `scripts/workflow.py`, `scripts/install.py`, `.github/workflows/release.yml` | workflow unit tests and smoke checks |

The matrix maps invariants to protection. It does not replace the focused implementation documents that explain each invariant.

## Related docs

- [Documentation coverage](documentation-coverage.md)
- [Testing and benchmarks](testing-and-benchmarks.md)
- [Release workflow](release-workflow.md)
- [Component architecture](../architecture/components.md)

## Notes

Update this matrix when an invariant, focused test location, component boundary, or release gate changes.
