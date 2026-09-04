# Behavioral Contract Matrix

Parent index: [Development Documentation](INDEX.md)

## Purpose

Map active Lexicon + Arcana invariants to focused tests and release gates.

## Overview

The matrix protects independently usable component boundaries, immutable publication, graph compatibility, deterministic release composition, and documentation/architecture governance. Retired Grimoire discovery behavior is historical evidence rather than a compatibility target.

## Contracts

| Contract | Primary verification owner |
| --- | --- |
| Lexicon owns language parsing and normalized semantic facts | Lexicon adapter, contract, scan, and publication tests |
| Lexicon snapshots are immutable, content-addressed, and crash-safe | Lexicon object-store, pending-publication, recovery, and transaction tests |
| Lexicon consumers are bounded and cannot corrupt a valid publication | `lexicon/internal/consumer/runner_test.go` and scan/publication tests |
| Arcana consumes verified Lexicon state rather than duplicating language parsers | Arcana Lexicon-ingestion and repository tests |
| Arcana preserves the consumed Lexicon snapshot identity | repository manifest and snapshot tests |
| Arcana publishes complete graph generations before replacing active state | repository/snapshot publication-failure tests |
| Arcana overlays validate base identity and compact without changing graph meaning | overlay and compaction tests |
| Exact graph traversal remains independent of optional semantic vectors | protocol/traversal tests and vector-disabled tests |
| `arcana.query.v1` capabilities required by consumers are negotiated before a combined build is accepted | root workflow protocol verification |
| Lexicon and Arcana remain independently installable | workflow packaging/install smoke tests |
| Combined release bundles contain only L+A executables, Lexicon adapters, installer, and legal metadata | `scripts/test_workflow.py` |
| Active release surfaces do not build, install, or publish Grimoire | Pitlord repository policy and workflow smoke tests |
| Root, Lexicon, and Arcana documentation trees pass without baselines | shared documentation policy and `scripts/check_docs.py` |
| Historical Grimoire benchmark artifacts remain evidence, not current product contracts | ADR 0006 and documentation ownership rules |

## Release gates

```bash
python scripts/workflow.py smoke
python scripts/workflow.py test
python .standards/docs_policy/check.py --repo .
python .standards/docs_policy/check.py --repo . --config docs-standard.lexicon.json
python .standards/docs_policy/check.py --repo . --config docs-standard.arcana.json
python scripts/check_docs.py
```

## Code map

| Matrix concern | Primary implementation or artifact | Protecting tests/gates |
| --- | --- | --- |
| Lexicon semantics/publication | `lexicon/adapters/`, `lexicon/internal/scan/`, `lexicon/internal/objectstore/` | Lexicon complete test matrix |
| Arcana graph publication/traversal | `arcana/src/repository/`, `arcana/src/storage/`, `arcana/src/snapshot/`, `arcana/src/protocol/` | Arcana Cargo test suite |
| Documentation/change impact | `.standards/docs_policy/`, `scripts/check_docs.py` | documentation-standard workflow |
| Architecture invariants | `tools/pitlord/` | Pitlord validation/check |
| Release packaging | `scripts/workflow.py`, `scripts/install.py`, `.github/workflows/release.yml` | workflow smoke and full test gate |

## Related docs

- [Documentation coverage](documentation-coverage.md)
- [Release workflow](release-workflow.md)
- [Component architecture](../architecture/components.md)
- [Architecture verification](architecture-verification.md)

## Notes

Update this matrix when an active invariant, focused test location, component boundary, or release gate changes. Retired Grimoire contracts belong in historical reports/ADRs, not this matrix.
