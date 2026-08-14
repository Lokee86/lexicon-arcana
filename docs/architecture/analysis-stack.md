# Lexicon–Arcana–Grimoire analysis stack

Parent index: [Architecture](INDEX.md)

## Purpose

This document defines the implemented cross-component lifecycle from repository source through Lexicon and Arcana to a Grimoire discovery response.

## Overview

Lexicon publishes normalized language facts, Arcana compiles and serves an immutable repository graph, and Grimoire coordinates aligned source, documentation, symbol, structural-expansion, and investigation state without absorbing component ownership.

This page describes the implemented lifecycle from repository source to a Grimoire discovery response.

## Ownership summary

```text
Lexicon  = language analysis and immutable normalized facts
Arcana   = immutable repository graph and deterministic graph queries
Grimoire = source/document retrieval, state coordination, progressive investigation
```

The three applications are co-located for product packaging, but they retain separate executables, state directories, formats, tests, and direct-use command surfaces.

## Publication pipeline

```text
repository files
  -> Lexicon adapter selection
  -> language adapters emit normalized facts
  -> Lexicon immutable objects and snapshot manifest
  -> .lexicon/CURRENT

.lexicon/CURRENT
  -> Arcana verifies the Lexicon snapshot and fact objects
  -> dense catalogue + packed forward/reverse graph
  -> optional immutable overlay for edge-only changes
  -> Arcana snapshot manifest
  -> .arcana/CURRENT

repository files + documentation + provider state
  -> Grimoire prepared source index
  -> Grimoire document index
  -> Grimoire reads Lexicon facts
  -> Grimoire queries Arcana
  -> independent evidence lanes in grimoire.discovery.v1
```

## Lexicon stage

Lexicon selects enabled languages, discovers relevant files, and runs one adapter per language. Adapters emit the normalized fact contract rather than provider-specific AST objects.

The published snapshot contains immutable content-addressed objects and a manifest recording source, adapter, schema, and configuration identities. `.lexicon/CURRENT` advances only after referenced state is durable.

Ordinary modified files may use impacted-file analysis. Structural changes, configuration changes, missing dependency information, or uncertain incremental results trigger complete analysis for the affected language. Output must remain deterministic across valid worker counts.

Lexicon may invoke registered post-publication consumers. Arcana registration uses this mechanism, but the Lexicon snapshot remains valid even when a consumer fails.

## Arcana stage

Arcana reads one immutable Lexicon snapshot, verifies every consumed object, compacts durable Lexicon identities into snapshot-local node IDs, and publishes:

- `graph.arcana` for packed forward and reverse adjacency;
- `catalogue.tsv` for node identity, path, kind, name, content identity, and source span;
- `unresolved.tsv` for unresolved-reference evidence;
- a manifest binding the graph to the consumed Lexicon snapshot.

When the node set is stable, Arcana can represent relationship changes as an immutable overlay over the packed base. Node additions/removals and shared fact changes rebuild the base. Compaction materializes a new base without mutating the source snapshot.

Arcana answers deterministic protocol operations such as symbol resolution, neighbors, paths, impact, call chains, unresolved references, statistics, snapshot differences, dead-symbol analysis, operational roles, and architecture summaries.

## Grimoire preparation stage

Grimoire owns two additional deterministic states:

- prepared source chunks under `.grimoire/`;
- documentation sections and optional document vectors under `.grimoire/knowledge/`.

`internal/repostate` reports and aligns all relevant identities. Its modes are:

- `current-only` — inspect without mutation;
- `refresh-if-needed` — rebuild only missing or stale required state;
- `force-refresh` — rerun Lexicon, Arcana synchronization, source preparation, and document indexing.

Vector state is inspected separately and is not silently built as part of ordinary repository preparation.

Repository freshness is deliberately broader than Grimoire's 2 MiB source-index eligibility cap. The preparation fingerprint covers Grimoire retrieval inputs plus the source extensions, build/dependency manifests, generic-language inputs, and ignore-control files that can change Lexicon output. A file may therefore be too large for Grimoire text indexing while still invalidating Lexicon and Arcana state.

## Discovery stage

A Grimoire search combines independent lanes:

| Lane | Owner and source |
| --- | --- |
| `exact_matches` | Grimoire literal source search |
| `source_matches` | Grimoire BM25 source retrieval |
| `document_matches` | Grimoire document BM25 plus optional document vectors |
| `symbol_matches` | Lexicon declarations and definitions |

Balanced discovery preserves per-lane budgets. Narrow discovery uses one compact code-evidence budget and normally returns handles without full source excerpts. Search reports structural expansion as deferred; `inspect`, query-ranked `trace`, and merged query-ranked `impact` expand selected evidence.

When a named investigation session is used, the ledger applies a second global hit ceiling across all returned lanes. Selection is round-robin by lane and then restored to original lane-local order, so one full lane cannot consume the complete session delta. Canonical evidence is pruned after hit selection and remains bounded by both evidence-count and serialized-byte limits.

The final response is provider-neutral. Lexicon and Arcana wire types do not escape as the public discovery schema.

## Snapshot alignment

Grimoire treats provider identities as explicit correctness boundaries:

- source handles bind to prepared-source identity;
- symbol handles bind to Lexicon snapshot identity;
- graph handles bind to Arcana snapshot identity;
- Arcana state records the Lexicon snapshot it consumed;
- investigation sessions preserve returned snapshot-qualified handles.

A stale handle or mismatched provider state must produce an explicit failure or warning rather than silently resolving against unrelated state. Runtime discovery exposes a provider snapshot only when that provider is current; a stale Arcana snapshot is omitted rather than queried as a degraded fallback.

## Degradation behavior

The stack is intentionally lane-tolerant:

- Grimoire source and document discovery can operate without Lexicon or Arcana.
- Lexicon symbol evidence can operate without Arcana.
- Arcana failure or staleness removes graph-backed evidence from the active prepared snapshot rather than exposing the previous graph, but does not invalidate exact source evidence.
- Missing document vectors preserve deterministic document BM25.
- Missing graph vectors do not affect ordinary Arcana traversal.

Warnings must identify unavailable or stale providers. An unrelated healthy lane remains usable.

## Operational paths

### Normal product path

```text
grimoire status --root . --refresh
grimoire search --root . --query "..."
grimoire inspect --root . --handle "..."
```

### Direct Lexicon path

```text
grimoire lexicon status --repo .
grimoire lexicon doctor --repo .
grimoire lexicon scan --repo .
```

### Direct Arcana path

```text
grimoire arcana sync --lexicon .lexicon --state .arcana
grimoire arcana query --graph <graph> --catalogue <catalogue> --name <symbol>
```

The namespaced commands forward to the owning executable and preserve its stdin, stdout, stderr, and exit status.

## Change ownership

| Change | Owning implementation |
| --- | --- |
| Parser, declaration, call, dataflow, dependency semantics | Lexicon adapter |
| Normalized node/edge/unresolved format | Lexicon specification and object store |
| Graph packing, overlays, compaction, traversal, protocol | Arcana |
| Source/document indexing and ranking | Grimoire |
| Provider freshness and coordinated preparation | Grimoire `internal/repostate` |
| Product-facing discovery response and progressive workflow | Grimoire `internal/agentquery` and `internal/agentruntime` |

See the [Grimoire maintainer map](maintainer-map.md), [Lexicon maintainer map](../../lexicon/docs/MAINTAINER_MAP.md), and [Arcana maintainer map](../../arcana/docs/MAINTAINER_MAP.md) for cross-document ownership routing.

## Code map

| Boundary | Primary implementation | Related tests |
| --- | --- | --- |
| Repository identity, freshness, locking, and provider coordination | `internal/repostate/` | `internal/repostate/*_test.go` |
| Prepared source and lexical state | `internal/index/`, `internal/lexical/`, `internal/retrieve/` | `internal/index/*_test.go`, `internal/lexical/*_test.go`, `internal/retrieve/*_test.go` |
| Lexicon snapshot loading and symbol evidence | `internal/lexiconfacts/`, `internal/structure/` | `internal/lexiconfacts/*_test.go` |
| Arcana protocol sessions and graph evidence | `internal/arcanagraph/`, `internal/structure/` | `internal/arcanagraph/*_test.go` |
| Documentation state and optional vectors | `internal/knowledge/`, `internal/knowledgevector/`, `internal/embedding/` | package-local `*_test.go` files |
| Discovery assembly and response shaping | `internal/agentruntime/`, `internal/agentquery/`, `internal/evidence/` | package-local `*_test.go` files |
| Persistent investigation state | `internal/investigation/` | `internal/investigation/*_test.go` |
| Command-level preparation | `internal/app/discovery_prepare.go`, `internal/app/query.go`, `internal/app/mcp.go` | `internal/app/discovery_test.go`, `internal/app/mcp_test.go` |

Lexicon adapters own language semantics, and Arcana owns graph compilation and storage. This document maps only Grimoire's coordination of those providers.

## Tests

Cross-component behavior is protected by `internal/repostate/*_test.go`, `internal/lexiconfacts/*_test.go`, `internal/arcanagraph/*_test.go`, `internal/agentruntime/*_test.go`, and the focused discovery tests under `internal/app/`. The root workflow also verifies the Lexicon and Arcana component suites.

## Related docs

- [Component architecture](components.md)
- [System overview](system-overview.md)
- [Lexicon reference](../reference/lexicon.md)
- [Arcana reference](../reference/arcana.md)

## Notes

This document owns the integrated lifecycle. Component-internal parsing, storage, and graph semantics remain owned by Lexicon and Arcana.
