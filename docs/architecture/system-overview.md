# System overview

Parent index: [Architecture](INDEX.md)

## Purpose

Define the active Lexicon + Arcana product architecture, data flow, state boundaries, failure behavior, and consumer model.

## Overview

Lexicon and Arcana provide deterministic repository intelligence without an intermediate discovery product:

```text
repository
    -> Lexicon semantic analysis
       symbols, calls, dataflow, dependencies, unresolved evidence
       immutable .lexicon snapshot
    -> Arcana graph compilation
       repository/call graph, traversal, impact, paths, architecture
       immutable .arcana snapshot
```

Consumers combine those analysis products with ordinary source/Git access according to their own needs. Higher-level agent orchestration belongs in Warlock or another consumer, not inside Lexicon or Arcana.

## Lexicon boundary

Lexicon owns language-specific semantic extraction and normalized repository facts. It publishes immutable, content-addressed snapshots and can be used independently.

Important properties:

- adapter-specific parsing behind a normalized fact contract;
- stable source and symbol identities;
- conservative unresolved evidence instead of guessed relationships;
- incremental analysis with complete-language fallback when needed;
- deterministic output and crash-safe publication.

## Arcana boundary

Arcana consumes one verified Lexicon snapshot and owns graph representation and query semantics.

Important properties:

- packed forward and reverse adjacency;
- immutable graph snapshots and overlays;
- exact graph traversal independent of embeddings;
- symbol/file resolution, neighbours, impact, paths, call chains, unresolved references, and architecture summaries;
- snapshot identity tied to the consumed Lexicon generation.

## Consumer model

```text
Agent / human / Warlock / Pitlord
    |
    +-> source + Git directly
    +-> Lexicon facts/snapshot when semantic ownership is needed
    +-> Arcana protocol when graph structure is needed
```

Consumers should use the cheapest authoritative surface for the question. A precise literal lookup does not need graph traversal. A call-path or dependency-impact question can use Arcana. A language-semantics question can use Lexicon facts. Material implementation conclusions should be verified against source.

## State directories

- `.lexicon/` — Lexicon configuration, immutable fact objects, manifests, and current snapshot pointer.
- `.arcana/` — Arcana graph snapshots, overlays, catalogue/unresolved metadata, and optional graph-vector state.
- `.grimoire/` — retired generated discovery state; migration/history only.

No active component mutates the other's state directly.

## Failure behavior

- A Lexicon publication is valid independently of Arcana consumer success.
- Arcana rejects corrupt, incompatible, or unverified Lexicon inputs.
- Arcana never silently treats a graph built from a different Lexicon snapshot as current for a newer generation.
- Missing optional graph vectors do not affect deterministic Arcana traversal.
- Consumers decide whether absence of Lexicon or Arcana evidence is fatal for their particular task; the components do not invent fallback source-retrieval semantics.

## Embeddings

Arcana's optional semantic graph index uses a generic external OpenAI-compatible embedding endpoint. Embeddings provide semantic entry points only; exact graph relationships remain authoritative.

The embedding runtime is not owned by Lexicon or Arcana. Warlock or another local service may provide it.

## Retired Grimoire responsibilities

The following are not part of the active system architecture:

- Grimoire source/document indexes;
- Grimoire BM25/exact discovery policy;
- heterogeneous evidence lanes and global response shaping;
- stable Grimoire handles and investigation sessions;
- Grimoire repository preparation/provider routing;
- Grimoire MCP and `grimoire.discovery.v1`.

They remain in the source tree temporarily until the implementation-removal pass and remain in historical documentation where needed to explain prior experiments.

## Code map

| Product boundary | Primary implementation | Related tests |
| --- | --- | --- |
| Lexicon CLI and lifecycle | `lexicon/cmd/lexicon/`, `lexicon/internal/cli/`, `lexicon/internal/scan/` | Lexicon package tests |
| Lexicon semantic analysis | `lexicon/adapters/` | adapter tests and semantic validation |
| Lexicon immutable publication | `lexicon/internal/objectstore/` | publication/recovery tests |
| Arcana repository compilation | `arcana/src/lexicon/`, `arcana/src/repository/` | ingestion/repository tests |
| Arcana graph storage and snapshots | `arcana/src/storage/`, `arcana/src/snapshot/` | storage/snapshot tests |
| Arcana query protocol | `arcana/src/protocol/` | protocol/traversal tests |

## Tests

The active architecture is verified primarily by Lexicon application, adapter, object-store, incremental-analysis, and snapshot tests; Arcana ingestion, packed-storage, snapshot, overlay, traversal, and protocol tests; and direct end-to-end Lexicon + Arcana benchmark/evaluation conditions.

Root tests that exist only to protect Grimoire's retired product behavior are transitional rather than future compatibility requirements.

## Related docs

- [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md)
- [Component architecture](components.md)
- [Analysis stack](analysis-stack.md)
- [Lexicon documentation](../../lexicon/docs/README.md)
- [Arcana documentation](../../arcana/docs/README.md)
