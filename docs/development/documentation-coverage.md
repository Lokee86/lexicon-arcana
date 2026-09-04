# Documentation Coverage

Parent index: [Development Documentation](INDEX.md)

## Purpose

Map active Lexicon + Arcana product ownership to canonical current documentation after Grimoire retirement.

## Overview

Current coverage contains two independently usable analysis products plus shared release, documentation, policy, and benchmark infrastructure. Historical Grimoire documentation is retained only as historical evidence and is not listed as an active owner here.

## Product and component coverage

| Boundary | Implementation | Canonical current owner |
| --- | --- | --- |
| Lexicon application and publication lifecycle | `lexicon/cmd/lexicon/`, `lexicon/internal/` | [Lexicon documentation](../../lexicon/docs/README.md) |
| Lexicon language semantics | `lexicon/adapters/`, `lexicon/spec/` | [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md), [Lexicon contracts](../../lexicon/spec/README.md) |
| Arcana application and graph lifecycle | `arcana/src/` | [Arcana documentation](../../arcana/docs/README.md) |
| Lexicon → Arcana ingestion | `arcana/src/lexicon/`, `arcana/src/repository/` | [Lexicon contract](../../arcana/docs/LEXICON_CONTRACT.md) |
| Shared L+A build/install/release and production agent skill | `scripts/workflow.py`, `scripts/install.py`, `.github/workflows/release.yml`, `skills/lexicon-arcana/SKILL.md` | [Release workflow](release-workflow.md), [Installation](../reference/installation.md) |
| Architecture enforcement | `tools/pitlord/` | [Architecture verification](architecture-verification.md) |
| Repository-analysis benchmark evidence | `evaluation/` | [Testing and benchmarks](testing-and-benchmarks.md), [Agent benchmark findings](agent-benchmark-findings.md) |
| Documentation governance | `.standards/`, `docs-standard*.json`, `scripts/check_docs.py` | [Documentation policy](../documentation-policy.md), [Documentation procedure](../documentation-procedure.md) |

## Machine-readable contracts

| Contract | Implementation owner | Canonical current owner |
| --- | --- | --- |
| Lexicon facts, immutable objects, snapshot manifests, and consumer definitions | `lexicon/spec/`, `lexicon/internal/objectstore/`, `lexicon/internal/consumer/` | [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md), [Lexicon application](../../lexicon/docs/APPLICATION.md) |
| `arcana.query.v1` | `arcana/src/protocol/` | [Arcana application](../../arcana/docs/APPLICATION.md), [Arcana architecture](../../arcana/docs/ARCHITECTURE.md) |
| Arcana repository, graph, snapshot, and overlay formats | `arcana/src/repository/`, `arcana/src/storage/`, `arcana/src/snapshot/` | [Repository snapshots](../../arcana/docs/repository-snapshots.md), [Arcana architecture](../../arcana/docs/ARCHITECTURE.md) |
| Optional Arcana semantic graph index | `arcana/src/vector/` | [Arcana vector index](../../arcana/docs/vector-index.md) |

`grimoire.discovery.v1`, Grimoire MCP framing, stable discovery handles, investigation sessions, document-vector manifests, and Grimoire repository-state contracts are retired historical interfaces. They have no active implementation owner.

## Stateful flows

| Flow | Canonical current owner |
| --- | --- |
| Lexicon analysis, immutable publication, recovery, and incremental scan | [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md) |
| Arcana verified ingestion, graph publication, overlays, and compaction | [Arcana architecture](../../arcana/docs/ARCHITECTURE.md), [Repository snapshots](../../arcana/docs/repository-snapshots.md) |
| Combined build, protocol verification, packaging, and installation | [Release workflow](release-workflow.md) |
| Higher-level agent/task/context orchestration | Warlock or another consumer; not owned by this repository |

## Code map

| Concern | Primary implementation | Related verification |
| --- | --- | --- |
| Shared documentation policy | `.standards/docs_policy/`, `docs-standard*.json` | documentation-standard CI and `scripts/check_docs.py` |
| Architecture policy | `tools/pitlord/policy.json`, `tools/pitlord/repository.json` | Pitlord validation/check in root workflow and CI |
| Release composition and installed L+A skill | `scripts/workflow.py`, `scripts/install.py`, `skills/lexicon-arcana/SKILL.md` | `scripts/test_workflow.py` and Pitlord policy |
| Lexicon | `lexicon/` | Lexicon package and adapter tests |
| Arcana | `arcana/` | Arcana Cargo tests and protocol capability check |
| Benchmark evidence | `evaluation/` | result-local summaries, grounding records, and benchmark reports |

## Related docs

- [Behavioral contract matrix](behavioral-contract-matrix.md)
- [Component architecture](../architecture/components.md)
- [Analysis stack](../architecture/analysis-stack.md)
- [Architecture verification](architecture-verification.md)

## Notes

Update this map when an active public command, machine-readable contract, stateful flow, release surface, or component boundary changes. Do not add retired Grimoire responsibilities back as active owners merely to preserve historical documentation shape.
