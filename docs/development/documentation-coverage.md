# Documentation Coverage

Parent index: [Development Documentation](INDEX.md)

## Purpose

This document maps Grimoire, Lexicon, and Arcana production ownership to canonical current documentation.

## Overview

Coverage includes commands, packages, component boundaries, stateful flows, machine-readable contracts, and independently usable component surfaces.

## Product commands and packages

| Boundary | Implementation | Canonical current owner |
| --- | --- | --- |
| Unified CLI and MCP orchestration | `cmd/grimoire/`, `internal/app/`, `internal/mcpserver/` | [CLI](../reference/cli.md), [Agent MCP](../reference/agent-mcp.md), [Operations and trust](../architecture/operations-and-trust.md) |
| Progressive discovery and sessions | `internal/agentruntime/`, `internal/agentquery/` | [Unified discovery contract](../reference/agent-query.md), [Analysis stack](../architecture/analysis-stack.md) |
| Prepared source state and retrieval | `internal/index/`, `internal/retrieve/` | [Indexing](../reference/indexing.md), [Prepared index](../architecture/prepared-index.md), [Operations and trust](../architecture/operations-and-trust.md) |
| Documentation indexing and retrieval | `internal/knowledge/`, `internal/knowledgevector/` | [Knowledge](../reference/knowledge.md), [Embedding model](../reference/embedding-model.md) |
| Lexicon fact integration | `internal/lexiconfacts/`, `internal/structure/` | [Lexicon reference](../reference/lexicon.md), [Component architecture](../architecture/components.md) |
| Arcana graph integration | `internal/arcanagraph/`, `internal/structure/` | [Arcana reference](../reference/arcana.md), [Component architecture](../architecture/components.md), [Operations and trust](../architecture/operations-and-trust.md) |
| Repository identity, conservative freshness, and aligned provider state | `internal/repostate/` | [Analysis stack](../architecture/analysis-stack.md), [System overview](../architecture/system-overview.md) |
| Persistent investigation evidence | `internal/investigation/` | [Agent MCP](../reference/agent-mcp.md), package README |
| Embedding runtime and vector boundary | `internal/embedding/`, `internal/vectorstore/` | [Embedding model](../reference/embedding-model.md), [Vector store](../reference/vector-store.md), [Operations and trust](../architecture/operations-and-trust.md) |
| Retrieval and agent outcome evaluation | `internal/knowledgeevaluation/`, `evaluation/agent_discovery/` | [Testing and benchmarks](testing-and-benchmarks.md), [Agent benchmark findings](agent-benchmark-findings.md) |

## Component coverage

| Component | Code root | Canonical current owner |
| --- | --- | --- |
| Lexicon language-analysis engine | `lexicon/` | [Lexicon documentation](../../lexicon/docs/README.md), [Root Lexicon reference](../reference/lexicon.md) |
| Arcana graph engine | `arcana/` | [Arcana documentation](../../arcana/docs/README.md), [Root Arcana reference](../reference/arcana.md) |
| Lodestone vector engine integration | pinned sibling `../lodestone` and Grimoire vector boundary | [Vector store](../reference/vector-store.md), [Component architecture](../architecture/components.md), [Operations and trust](../architecture/operations-and-trust.md) |

## Machine-readable contracts

| Contract | Implementation owner | Canonical current owner |
| --- | --- | --- |
| `grimoire.discovery.v1` request, response, handles, lanes, warnings, and assessment | `internal/agentquery/`, `internal/agentruntime/` | [Unified discovery contract](../reference/agent-query.md) |
| Grimoire MCP JSON-RPC, stdio framing, admission, cancellation, and audit records | `internal/app/mcp.go`, `internal/app/mcp_audit.go`, `internal/mcpserver/` | [Agent MCP](../reference/agent-mcp.md), [Operations and trust](../architecture/operations-and-trust.md) |
| Lexicon `facts-v1`, immutable objects, snapshot manifests, and consumer definitions | `lexicon/spec/`, `lexicon/internal/objectstore/`, `lexicon/internal/consumer/` | [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md), [Lexicon application](../../lexicon/docs/APPLICATION.md) |
| `arcana.query.v1` request and response protocol | `arcana/src/protocol/` | [Arcana application](../../arcana/docs/APPLICATION.md), [Arcana architecture](../../arcana/docs/ARCHITECTURE.md) |
| Shared packed graph, graph manifest, and overlay formats; Arcana repository manifest binding | `arcana-graph/src/storage/`, `arcana-graph/src/snapshot/`, `arcana/src/repository/` | [Repository snapshots](../../arcana/docs/repository-snapshots.md), [Arcana architecture](../../arcana/docs/ARCHITECTURE.md) |
| Grimoire document-vector and Arcana graph-vector manifests | `internal/knowledgevector/`, `arcana/src/vector/` | [Vector store](../reference/vector-store.md), [Arcana vector index](../../arcana/docs/vector-index.md) |

## Stateful flows

| Flow | Canonical current owner |
| --- | --- |
| Repository preparation, conservative source/provider freshness, and snapshot alignment across Grimoire, Lexicon, Arcana, and documentation | [Analysis stack](../architecture/analysis-stack.md) |
| Exact, BM25, document, and symbol lane assembly with deferred structural expansion | [Unified discovery contract](../reference/agent-query.md) |
| Stable snapshot-qualified handles and inspection | [Agent MCP](../reference/agent-mcp.md) |
| MCP negotiation, bounded admission, cancellation, and audit privacy | [Agent MCP](../reference/agent-mcp.md), [Operations and trust](../architecture/operations-and-trust.md) |
| Investigation session reuse and prior-evidence compaction | [Agent MCP](../reference/agent-mcp.md) |
| Lexicon immutable object and snapshot lifecycle | [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md) |
| Arcana graph snapshot, overlay, and compaction lifecycle | [Arcana architecture](../../arcana/docs/ARCHITECTURE.md), [Repository snapshots](../../arcana/docs/repository-snapshots.md) |
| Document-vector and graph-vector storage through Lodestone | [Vector store](../reference/vector-store.md), [Arcana vector index](../../arcana/docs/vector-index.md) |

## Code map

| Concern | Primary implementation | Related verification |
| --- | --- | --- |
| Shared documentation policy engine | `.standards/docs_policy/` | configured checks for root, Lexicon, and Arcana trees |
| Repository documentation configuration | `docs-standard.json`, `docs-standard.lexicon.json`, `docs-standard.arcana.json` | zero-debt shared checks and component-specific change-impact gates |
| Grimoire-specific required documents and links | `scripts/check_docs.py` | `python scripts/check_docs.py` through the root workflow |
| Architectural invariants and dependency direction | `tools/pitlord/policy.json`, `tools/pitlord/repository.json` | Pitlord validation and repository checks through the root workflow and CI |
| Workflow integration, pinned Lodestone identity, and Arcana protocol compatibility | `scripts/workflow.py` | `scripts/test_workflow.py`, exact source verification, built Arcana capability negotiation |
| CI enforcement | `.github/workflows/documentation-standard.yml` | push and pull-request architecture/documentation gates |

Coverage tables identify canonical owners. They do not substitute for prose, state/lifecycle explanation, focused code maps, or tests in those owners.

## Related docs

- [Behavioral contract matrix](behavioral-contract-matrix.md)
- [Component architecture](../architecture/components.md)
- [Analysis stack](../architecture/analysis-stack.md)
- [Operations and trust boundaries](../architecture/operations-and-trust.md)
- [Architecture verification](architecture-verification.md)

## Notes

Update this map whenever an independent production flow, public command, machine-readable contract, package responsibility, or component boundary changes.
