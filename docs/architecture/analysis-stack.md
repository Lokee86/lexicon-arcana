# Lexicon–Arcana analysis stack

Parent index: [Architecture](INDEX.md)

## Purpose

Define the active deterministic lifecycle from repository source through Lexicon semantic analysis to Arcana repository-graph queries.

## Overview

```text
Lexicon = polyglot semantic analysis + immutable normalized facts
Arcana  = immutable repository/call graph + deterministic graph queries
```

Grimoire is retired as a downstream discovery layer. Consumers use Lexicon and Arcana directly and use ordinary source/Git tools for exact implementation evidence.

## Publication pipeline

```text
repository files
  -> Lexicon language discovery and adapter selection
  -> adapters emit normalized facts
  -> Lexicon immutable objects + snapshot manifest
  -> .lexicon/CURRENT

.lexicon/CURRENT
  -> Arcana verifies the snapshot and consumed fact objects
  -> dense catalogue + packed forward/reverse graph
  -> optional immutable overlay for edge-only changes
  -> Arcana snapshot manifest
  -> .arcana/CURRENT

.lexicon/CURRENT + .arcana/CURRENT
  -> humans, agents, Warlock, Pitlord, and other consumers
```

## Lexicon stage

Lexicon selects enabled languages, discovers relevant files, and runs the owning semantic adapter for each language surface. Adapters emit the normalized fact contract rather than leaking parser-specific AST objects.

The published snapshot contains immutable content-addressed objects and a manifest recording source, adapter, schema, and configuration identities. `.lexicon/CURRENT` advances only after referenced state is durable.

Incremental analysis is conservative. Structural/configuration changes, missing dependency information, or uncertain impact may trigger complete analysis for the affected language. Output must remain deterministic across valid worker counts.

Lexicon may invoke registered post-publication consumers. Arcana registration uses this mechanism, but consumer failure does not invalidate the published Lexicon snapshot.

## Arcana stage

Arcana reads and verifies one immutable Lexicon snapshot, compacts durable Lexicon identities into snapshot-local node IDs, and publishes:

- `graph.arcana` — packed forward/reverse adjacency;
- `catalogue.tsv` — graph IDs mapped to Lexicon identities, paths, kinds, names, content identities, and spans;
- `unresolved.tsv` — unresolved-reference evidence;
- a repository manifest bound to the consumed Lexicon snapshot.

When the node set remains stable, Arcana can represent relationship changes as an immutable overlay over the packed base. Node-set or shared-fact changes rebuild the base. Compaction creates a new immutable base without mutating the source snapshot.

Arcana serves deterministic graph operations including symbol/file resolution, neighbours, impact, paths, shortest call chains, unresolved references, statistics, snapshot differences, operational roles, and architecture summaries.

## Normal operational path

First use:

```text
lexicon init --repo /path/to/repository
arcana sync --lexicon /path/to/repository/.lexicon --state /path/to/repository/.arcana --register
```

Later updates:

```text
lexicon scan --repo /path/to/repository
```

When Arcana is registered as a consumer, a successful Lexicon publication invokes the same one-shot Arcana synchronization path. Direct synchronization remains available:

```text
arcana sync --lexicon .lexicon --state .arcana
```

## Query path

Lexicon exports are useful when a consumer needs normalized semantic facts directly. Arcana is the graph query surface.

Typical Arcana protocol operations:

```json
{"id":1,"op":"search_nodes","query":"session creation","limit":12}
{"id":2,"op":"resolve_symbol","name":"CreateSession","limit":12}
{"id":3,"op":"neighbors","node_id":123,"direction":"incoming","limit":20}
{"id":4,"op":"impact","node_id":123,"max_depth":3,"limit":40}
{"id":5,"op":"shortest_call_chain","from_node_id":123,"to_node_id":456,"max_depth":8}
```

Graph evidence identifies likely owners and structural relationships. Exact source remains implementation authority for conclusions about current behavior.

## Snapshot alignment

- Lexicon state is authoritative only through a verified published snapshot.
- Each Arcana snapshot records the exact Lexicon snapshot it consumed.
- Consumers must not combine unrelated Lexicon and Arcana generations as though they were aligned.
- Corrupt, missing, or incompatible state is rejected rather than repaired in place by another component.

## Optional semantic graph index

Arcana may explicitly build a semantic entry-point index against an OpenAI-compatible embedding endpoint. Embeddings are optional and do not replace deterministic graph relationships or ordinary protocol operations.

The embedding service is external to Arcana. Warlock or another local service may provide it; Grimoire is not required.

## Change ownership

| Change | Owner |
| --- | --- |
| Parsing, declarations, calls, dataflow, dependencies | Lexicon adapter |
| Normalized node/edge/unresolved contracts | Lexicon |
| Semantic object and snapshot publication | Lexicon |
| Graph ingestion, packing, overlays, compaction | Arcana |
| Neighbours, paths, impact, call chains, architecture queries | Arcana |
| Agent/context/task orchestration | Warlock or another consumer |
| Literal search, file reads, Git/history | Ordinary development tools |

## Historical Grimoire layer

The former Grimoire prepared indexes, heterogeneous discovery lanes, stable handles, sessions, MCP, and provider routing are historical/transitional implementation. They are not part of the active stack and will be removed from active build/release paths under [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md).

Historical benchmark results remain evidence and should retain their original condition names.

## Code map

| Boundary | Primary implementation | Related tests |
| --- | --- | --- |
| Lexicon application and scan lifecycle | `lexicon/cmd/lexicon/`, `lexicon/internal/cli/`, `lexicon/internal/scan/` | package-local Go tests |
| Lexicon semantic adapters | `lexicon/adapters/` | adapter-owned tests and evaluation corpora |
| Lexicon immutable objects and snapshots | `lexicon/internal/objectstore/` | object-store and publication tests |
| Arcana Lexicon ingestion | `arcana/src/lexicon/`, `arcana/src/repository/` | Lexicon ingestion and repository tests |
| Arcana packed graph state | `arcana/src/storage/`, `arcana/src/snapshot/` | storage, overlay, snapshot, and compaction tests |
| Arcana graph protocol | `arcana/src/protocol/` | protocol and traversal tests |
| Optional Arcana semantic index | `arcana/src/vector/` | vector index/search tests |

## Tests

The active stack is protected by Lexicon application, adapter, object-store, incremental-analysis, and publication tests plus Arcana ingestion, repository, storage, snapshot, traversal, and protocol tests. Direct Lexicon + Arcana benchmark conditions provide end-to-end consumer evidence. Grimoire-specific discovery tests are transitional retirement coverage, not future compatibility requirements.

## Related docs

- [Component architecture](components.md)
- [System overview](system-overview.md)
- [Lexicon architecture](../../lexicon/docs/ARCHITECTURE.md)
- [Arcana Lexicon contract](../../arcana/docs/LEXICON_CONTRACT.md)
