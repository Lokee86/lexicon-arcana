# System overview

Parent index: [Architecture](INDEX.md)

## Purpose

This document defines Grimoire's current product architecture, discovery lanes, repository preparation, failure behavior, and state boundaries.

## Overview

Grimoire presents source retrieval, documentation retrieval, Lexicon symbols, and Arcana relationships through one progressive interface while preserving independent evidence classes and explicit provider degradation.

Grimoire contains three independently owned engines presented through one repository-discovery interface:

- Grimoire owns source and documentation discovery, stable handles, progressive investigation, and state orchestration.
- Lexicon owns language analysis and normalized symbols and relationships.
- Arcana owns packed repository graphs and graph queries.

Source co-location does not merge runtime state or domain ownership. See [Component architecture](components.md).

## Repository preparation

```text
Repository source
  -> Grimoire eligibility and ignore rules
  -> immutable prepared source snapshot
  -> exact lookup and BM25 postings

Repository source
  -> Lexicon language adapters
  -> immutable Lexicon facts and snapshot
  -> Arcana graph compilation
  -> immutable Arcana graph snapshot

Repository documentation
  -> independent document index
  -> section handles, freshness metadata, and BM25 postings
  -> optional documentation vectors
```

Source discovery does not require embeddings. Lexicon, Arcana, and documentation vectors may degrade independently.

## Query-time discovery

```text
User or agent query
  -> exact source discovery             -> exact_matches
  -> source BM25 discovery              -> source_matches
  -> document BM25 / optional vectors   -> document_matches
  -> Lexicon symbol resolution          -> symbol_matches
  -> structural expansion deferred to trace/impact
```

Balanced search preserves independent lane limits; narrow search applies one combined code-evidence budget. Search does not traverse graph neighbors automatically. This preserves heterogeneous ranked evidence and lets the agent choose a stable handle before structural expansion.

Follow-up operations use stable handles:

```text
inspect(handle) -> exact source or document evidence
trace(handle)   -> bounded graph paths
impact(handle)  -> bounded incoming or outgoing dependents
```

## Ownership boundaries

### Grimoire application

`internal/app` owns CLI and MCP commands, repository-state preparation, timeout and fallback policy, and the flattened discovery contract.

`internal/agentruntime` combines source/symbol/relationship responses with the separately indexed documentation lane and optional investigation sessions.

`internal/agentquery` owns provider-neutral orient, search, trace, impact, and source-inspection behavior.

### Source state

`internal/index` owns repository traversal, chunking, immutable object reuse, exact lookup inputs, lexical postings, and prepared snapshot publication. Generated state, Git metadata, and nested worktrees are excluded.

`internal/retrieve` owns exact and BM25 source discovery. It does not rank documentation or structural graph evidence.

### Documentation state

`internal/knowledge` owns documentation discovery, section extraction, stable citation handles, freshness metadata, BM25 ranking, and code links.

`internal/knowledgevector` optionally supplements document ranking. Document scores never enter source, symbol, or relationship ranking.

### Lexicon

`lexicon/` owns language extraction, normalized source identities, symbols, relationships, immutable analysis objects, snapshot publication, and adapter execution.

`internal/lexiconfacts` is Grimoire's read-only integration boundary for immutable Lexicon exports.

### Arcana

`arcana/` owns Lexicon snapshot ingestion, repository graph construction, packed graph storage, overlays, compaction, neighbors, paths, impact, unresolved references, and the graph protocol.

`internal/arcanagraph` is Grimoire's protocol client. It resolves discovered symbols and asks Arcana for direct relationships or bounded graph expansions without copying graph ownership into Grimoire.

### Investigation sessions

`internal/investigation` records returned nodes, source ranges, documents, and graph paths. Repeated evidence is represented by prior handles rather than replayed content.

### Embeddings and vector storage

`internal/embedding` owns the configured embedding model and managed runtime. `internal/vectorstore` is Grimoire's Lodestone compatibility boundary. Embeddings are optional for documentation and graph semantic entry points; they are not required for source discovery.

## Failure and fallback behavior

- Missing documentation vectors leave the document lane on BM25.
- Missing or stale Arcana state omits Arcana from the prepared query snapshot; structural graph traversal is unavailable until Arcana is current.
- Missing Lexicon and Arcana state leaves exact and source discovery available.
- Provider failures are reported in `warnings` and do not discard unrelated evidence lanes.
- Stale handles are rejected rather than silently rediscovered.
- Arcana state remains bound to the Lexicon snapshot it consumed.

## State directories

- `.grimoire/` — prepared source state, document state, and investigation sessions.
- `.grimoire/knowledge/` — document index and optional vector state.
- `.lexicon/` — immutable Lexicon analysis state.
- `.arcana/` — Arcana graph state and optional semantic graph indexes.

Each format remains independently versioned and owned. Integration occurs through manifests, immutable exports, and explicit protocols.

## Retired context-package path

The former package compiler attempted to merge and token-fit source and structural evidence before an agent could investigate it. The command and its assembly, compiler, curation, query-shape, diff-context, graph-ranking, and source-evaluation code have been removed. Historical reports remain calibration records only.

## Code map

| Runtime stage | Primary implementation | Related tests |
| --- | --- | --- |
| CLI/MCP entry and dispatch | `cmd/grimoire/main.go`, `internal/app/run.go`, `internal/app/query.go`, `internal/app/mcp.go` | `internal/app/run_test.go`, `internal/app/discovery_test.go`, `internal/app/mcp_test.go` |
| Repository preparation | `internal/repostate/`, `internal/app/discovery_prepare.go` | `internal/repostate/*_test.go`, `internal/app/discovery_test.go` |
| Source indexing and retrieval | `internal/index/`, `internal/lexical/`, `internal/retrieve/` | package-local `*_test.go` files |
| Documentation retrieval | `internal/knowledge/`, `internal/knowledgevector/` | package-local `*_test.go` files |
| Symbol and relationship providers | `internal/lexiconfacts/`, `internal/arcanagraph/`, `internal/structure/` | provider package tests |
| Unified discovery response | `internal/agentruntime/`, `internal/agentquery/`, `internal/evidence/` | package-local `*_test.go` files |
| Session persistence | `internal/investigation/` | `internal/investigation/*_test.go` |

The retired context-package assembly path is documentation history only; it is not an active implementation owner.

## Tests

The active architecture is protected by command and preparation tests under `internal/app/`, state tests under `internal/repostate/`, retrieval and provider package tests, investigation-session tests, and the end-to-end agent-discovery evaluation harness.

## Related docs

- [Component architecture](components.md)
- [Analysis stack](analysis-stack.md)
- [Unified discovery contract](../reference/agent-query.md)
- [Current limitations](../limits/current-limitations.md)

## Notes

The retired context-package pipeline is historical only and is not a current implementation or planning owner.
