# Arcana implementation architecture

Parent index: [Arcana Documentation](README.md)

## Purpose

This document defines Arcana's ownership, dependency direction, ingestion, compilation, packed storage, immutable snapshots, protocol, vectors, concurrency, failure boundaries, and invariants.

## Overview

Arcana consumes language-neutral facts and owns deterministic repository-graph state. It does not parse source languages or own higher-level repository-discovery, context, or agent workflow policy.

This document describes the architecture implemented by the current Arcana source and covered by its focused tests. It is an ownership and dependency map, not a roadmap or a file-format specification.

Detailed contracts:

- [Lexicon ingestion boundary](LEXICON_CONTRACT.md)
- [Repository snapshots and incremental updates](repository-snapshots.md)
- [Semantic graph index](vector-index.md)
- [Application and operations reference](APPLICATION.md)
- [Maintainer map](MAINTAINER_MAP.md)

## Scope and dependency direction

Arcana owns the language-neutral repository graph after Lexicon has published facts. It does not parse source languages or run adapters. Its production data flow is:

```text
Lexicon immutable store
  -> lexicon verification and decoding
  -> RepositoryFacts
  -> deterministic repository compilation
  -> packed graph plus catalogue/unresolved/fact metadata
  -> graph snapshot
  -> repository snapshot
  -> exact JSONL protocol
       or
     explicit optional vector index/search
```

The binary command modules orchestrate these library owners; library modules do not depend on the CLI. The main inward dependencies are:

| Owner | Owns | Depends on |
| --- | --- | --- |
| `lexicon` | Lexicon snapshot/object verification, decoding, identity conversion, compatibility warnings | `repository` fact models and path normalization |
| `repository` facts/compiler | Stable fact models, dense compilation, catalogue, ownership partitioning, incremental planning | Dense graph primitives; incremental planning emits `snapshot::OverlayChanges` |
| `storage` | Immutable packed bytes and forward/reverse adjacency readers | Dense graph primitives only |
| `snapshot` | Graph manifests, packed-base composition, overlays, visible reads, compaction | `storage` and dense graph primitives |
| `repository` snapshot | Complete graph-plus-metadata generation publication and validation | `snapshot`, catalogue, facts, and unresolved records |
| `protocol` | Validated, repeated exact graph queries and JSON response shapes | One opened repository snapshot |
| `vector` | Optional graph documents, embedding cache, vector index, and semantic search | One opened repository snapshot and an external embedder |
| CLI orchestration | Import, update, sync, protocol, and vector command lifecycles | The library owners above |

`NodeId`, `EdgeKind`, `Edge`, and `GraphDataset` are defined in `synthetic`, re-exported from the Arcana library root as reusable graph primitives, and shared by compilation, storage, snapshots, and external library consumers. `traversal` exposes bounded repository-agnostic graph traversal over caller-supplied adjacency. Synthetic generation and benchmarking are not part of the Lexicon-to-query runtime path.

Evidence: [`lib.rs`](../src/lib.rs), [`repository/mod.rs`](../src/repository/mod.rs), [`storage/mod.rs`](../src/storage/mod.rs), [`snapshot/mod.rs`](../src/snapshot/mod.rs), [`protocol/mod.rs`](../src/protocol/mod.rs), [`vector/mod.rs`](../src/vector/mod.rs), and [`synthetic/mod.rs`](../src/synthetic/mod.rs).

## Implemented ownership boundaries

### Lexicon ingestion

`lexicon` consumes an immutable Lexicon store. It resolves `CURRENT`, verifies the content-addressed snapshot manifest and every referenced object, checks versions and cross-object metadata, decodes binary v1 or legacy canonical JSON objects, normalizes repository paths, and assembles one complete `RepositoryFacts` value. It does not invoke adapters.

Lexicon SHA-256 node identities remain the external identities recorded in the catalogue. Ingestion compacts them to `NodeKey`, rejects compact-key collisions, and records file/shared object identities for later change detection. A language-level shared-object identity change is distinct from added, changed, and removed file-object paths.

Evidence: [`lexicon/snapshot.rs`](../src/lexicon/snapshot.rs), [`lexicon/records.rs`](../src/lexicon/records.rs), [`lexicon/tests.rs`](../src/lexicon/tests.rs), [`lexicon/binary_tests.rs`](../src/lexicon/binary_tests.rs), and [`cli_sync_tests.rs`](../src/cli_sync_tests.rs). The detailed producer/consumer contract is [LEXICON_CONTRACT.md](LEXICON_CONTRACT.md).

### Repository facts and compilation

`repository` owns language-neutral nodes, relationships, unresolved references, source spans, repository-relative paths, and their canonical persisted text forms. Compilation validates conflicting nodes and missing endpoints, orders nodes by `NodeKey`, assigns dense snapshot-local `NodeId` values, maps relations to stable nonzero edge codes, canonicalizes repeated relationship occurrences into one reachability edge, and builds the dense-ID catalogue.

File-scoped update planning partitions facts by owner, replaces only declared file partitions, recompiles the visible fact set, and compares the complete stable-key-to-dense-ID map. It produces edge additions and removals relative to the original packed base only when that map and the packed node count remain unchanged.

Evidence: [`repository/model.rs`](../src/repository/model.rs), [`repository/compiler.rs`](../src/repository/compiler.rs), [`repository/ownership.rs`](../src/repository/ownership.rs), [`repository/incremental.rs`](../src/repository/incremental.rs), and their focused `compiler_catalogue_tests.rs`, `ownership_tests.rs`, and `incremental_tests.rs` files under [`repository/`](../src/repository/).

### Packed storage

`storage` owns the immutable packed graph format. The writer canonicalizes the dataset, emits forward and reverse adjacency sections with counts and checksums, synchronizes a temporary file, and refuses to replace an existing packed path. The reader loads one immutable shared byte buffer, validates the header, layout, file and payload lengths, checksums, offset tables, endpoint bounds, and adjacency ordering, then exposes forward and reverse iterators.

Packed storage contains dense graph topology, not durable external identities or source metadata. Those remain in repository artifacts bound by `repository.manifest`.

Evidence: [`storage/writer.rs`](../src/storage/writer.rs), [`storage/reader.rs`](../src/storage/reader.rs), [`storage/tests.rs`](../src/storage/tests.rs), and [`storage/corruption_tests.rs`](../src/storage/corruption_tests.rs).

### Immutable graph snapshots and overlays

`snapshot` composes a packed base with zero or one overlay under `graph.manifest`. An overlay contains canonical added-edge and removed-edge operations and is bound to the base node count, edge count, and dataset checksum. Opening a graph snapshot validates the base, overlay, visible edge count, visible dataset checksum, and derived snapshot identity before serving reads.

Without an overlay, visible-neighbor iterators borrow packed adjacency directly. With an overlay, reads merge base neighbors, removals, and additions in either direction. Overlays change edges only; they cannot change the dense node set. Incremental generations therefore keep the original packed base and write one cumulative overlay relative to it.

Evidence: [`snapshot/graph.rs`](../src/snapshot/graph.rs), [`snapshot/overlay.rs`](../src/snapshot/overlay.rs), [`snapshot/overlay_validation.rs`](../src/snapshot/overlay_validation.rs), [`snapshot/graph_tests.rs`](../src/snapshot/graph_tests.rs), [`snapshot/overlay_tests.rs`](../src/snapshot/overlay_tests.rs), and [`cli_update_tests.rs`](../src/cli_update_tests.rs).

### Repository snapshots

A repository snapshot binds `graph.manifest`, `catalogue.tsv`, `unresolved.tsv`, and `facts.tsv` under `repository.manifest`. Publication validates graph and metadata consistency and writes the repository manifest last. Opening rechecks component checksums and cross-artifact invariants before exposing graph, catalogue, facts, or unresolved records.

Full compilation writes a new packed base and base-only graph manifest. Changed-file updates copy the original packed base, optionally write a cumulative overlay, and publish new metadata describing the visible generation. Neither path mutates its source generation.

Evidence: [`repository/repository_snapshot.rs`](../src/repository/repository_snapshot.rs), [`repository/repository_snapshot_validation.rs`](../src/repository/repository_snapshot_validation.rs), [`repository/repository_snapshot_tests.rs`](../src/repository/repository_snapshot_tests.rs), [`cli_commands.rs`](../src/cli_commands.rs), and [`cli_update.rs`](../src/cli_update.rs). Artifact details are in [repository-snapshots.md](repository-snapshots.md).

### Compaction

Compaction is a library operation owned by `snapshot::compaction`. It opens a source graph snapshot, materializes its visible forward edges, writes a new immutable packed base, verifies visible edge-count and dataset-checksum equivalence, and publishes a new base-only manifest. It removes incomplete output on verification/publication failure and never modifies the source snapshot. There is no compaction CLI command in the current command surface.

Evidence: [`snapshot/compaction.rs`](../src/snapshot/compaction.rs) and [`snapshot/compaction_tests.rs`](../src/snapshot/compaction_tests.rs).

### Query protocol

`protocol` opens and validates one complete repository snapshot at startup, transfers its graph/catalogue/unresolved components into a `ProtocolSnapshot`, and serves repeated JSON Lines requests against that fixed snapshot. Every response uses `arcana.query.v1`, echoes the parseable request ID, and is either a result or a structured error. A request error does not terminate the stdin/stdout loop.

Operations are routed to narrow owners for node lookup, neighbors, unresolved records, bounded traversal/path/analysis, architecture summaries, statistics, snapshot diff, and graph export. They query `GraphSnapshot`, so they see visible overlay state rather than only the packed base.

Evidence: [`protocol/session.rs`](../src/protocol/session.rs), [`protocol/server.rs`](../src/protocol/server.rs), [`protocol/response.rs`](../src/protocol/response.rs), the narrow query modules under [`protocol/`](../src/protocol/), and [`protocol/tests.rs`](../src/protocol/tests.rs).

### Graph export

`protocol::graph_export` owns graph export rather than the storage layer. It normalizes an optional path prefix, selects a bounded catalogue page, adds validated pinned nodes without advancing that page, and emits only visible edges whose endpoints are both in the returned node set. Nodes follow catalogue/page order; edges are sorted deterministically. Export uses overlay-aware forward adjacency and caps a page at 100,000 nodes.

Evidence: [`protocol/graph_export.rs`](../src/protocol/graph_export.rs) and the paged, pinned-node, and visible-overlay export cases in [`protocol/tests.rs`](../src/protocol/tests.rs).

### Optional vectors

`vector` is an explicit consumer of a validated repository snapshot; it is not in sync, packed storage, snapshot opening, or exact protocol queries. It renders deterministic bounded documents for eligible graph entry points, keys reusable cache objects by document and embedding contract, embeds cache misses through an external `Embedder`, and materializes a snapshot/model/policy-bound index.

Index build validates exact existing indexes before reuse, serializes cache/index publication, writes and verifies a temporary index, and rejects publication if Arcana `CURRENT` changes. Search takes a shared index lock, validates current snapshot/model/policy identity, embeds the query, validates records and finite vector values while scoring, sorts by descending score with node key as the tie-breaker, and rechecks `CURRENT`. Semantic hits are entry points; authoritative relationships remain in the exact graph protocol.

Evidence: [`vector/documents.rs`](../src/vector/documents.rs), [`vector/cache.rs`](../src/vector/cache.rs), [`vector/build.rs`](../src/vector/build.rs), [`vector/index.rs`](../src/vector/index.rs), [`vector/search.rs`](../src/vector/search.rs), [`vector/documents_tests.rs`](../src/vector/documents_tests.rs), and [`vector/index_tests.rs`](../src/vector/index_tests.rs). The storage and embedding contract is described in [vector-index.md](vector-index.md); versioned constants in current source remain authoritative.

## Concurrency and publication

Arcana publishes immutable generations and changes only pointers or replaceable auxiliary indexes:

- `sync` takes a non-blocking exclusive `<state>/LOCK`, builds under a temporary snapshot directory, renames the verified directory to its digest path, and atomically replaces `<state>/CURRENT` last.
- Packed graphs, overlays, graph manifests, and repository manifests use create-new/immutable write paths and refuse in-place replacement.
- Vector builds take an exclusive cache build lock and an exclusive per-snapshot/per-identity index lock. Semantic search takes the same index lock in shared mode.
- Vector build and search also compare `CURRENT` at critical boundaries so one operation cannot claim or combine two graph snapshots.

The sync lock and vector locks are separate domains: sync protects managed graph-generation publication; vector locks protect optional cache/index materialization and reading.

Evidence: [`cli_sync.rs`](../src/cli_sync.rs), [`cli_sync_state.rs`](../src/cli_sync_state.rs), inline lock/atomic-replacement tests in `cli_sync_state.rs`, and the concurrency, reuse, interruption, and rollback cases in [`vector/index_tests.rs`](../src/vector/index_tests.rs).

## Degradation and failure boundaries

Arcana degrades only where the conversion is explicit and recorded:

- An unknown Lexicon node kind becomes `symbol`.
- An edge or unresolved record with an unknown relation is skipped because Arcana has no safe relation semantics to assign.
- An unknown unresolved-reason label is preserved verbatim.
- These compatibility degradations are deduplicated into warnings, printed by `sync`, and persisted in the immutable generation as `compatibility.warnings`.

Malformed required fields, invalid IDs/paths, unsupported storage versions, hash/checksum mismatches, conflicting identities, invalid graph operations, and cross-artifact inconsistencies are hard errors. An invalid repository snapshot prevents protocol startup; an invalid individual request receives an error response while the already-valid session continues.

Incremental inability has command-specific behavior. `update-facts` reports a rebuild-required error when the dense node mapping changes. Managed `sync` falls back to a full rebuild for shared-object changes, unusable prior state, node-set changes, or failed incremental planning. A missing, stale, or corrupt optional vector index affects vector build/search, not deterministic graph synchronization or protocol queries.

Evidence: [`lexicon/records.rs`](../src/lexicon/records.rs), [`repository/incremental.rs`](../src/repository/incremental.rs), [`cli_sync.rs`](../src/cli_sync.rs), [`protocol/session.rs`](../src/protocol/session.rs), storage/snapshot corruption tests, [`cli_sync_tests.rs`](../src/cli_sync_tests.rs), and [`vector/index_tests.rs`](../src/vector/index_tests.rs).

## End-to-end data flows

### Full managed synchronization

1. `cli_sync` acquires the state lock and asks `lexicon` to resolve and verify Lexicon `CURRENT`.
2. `lexicon` decodes objects into `RepositoryFacts` plus compatibility warnings.
3. `repository::compiler` creates the deterministic dense dataset, node mapping, catalogue, and unresolved records.
4. `storage` writes `graph.arcana`; `snapshot` publishes a base-only `graph.manifest`.
5. Repository publication writes metadata and publishes `repository.manifest` after validation.
6. `cli_sync` writes Lexicon sidecars, renames the temporary generation into its immutable digest directory, and atomically replaces Arcana `CURRENT`.

### Incremental managed synchronization

1. `cli_sync` opens the prior Lexicon and Arcana snapshots and compares shared/file object identities.
2. With unchanged shared objects, changed file paths drive ownership-based fact replacement.
3. Incremental planning recompiles the candidate facts and requires the stable-key-to-dense-ID map to match.
4. Edge differences are computed against the original packed base and written as one cumulative overlay.
5. New graph and repository manifests bind the base, overlay, and replacement metadata; normal immutable publication then applies.
6. If the path is not valid for an overlay, managed sync rebuilds instead.

### Exact query and graph export

1. Protocol startup opens `repository.manifest` and validates every bound component.
2. Each JSONL request is parsed and routed while the same `ProtocolSnapshot` remains pinned.
3. Query owners combine catalogue metadata with visible graph adjacency.
4. Graph export pages catalogue nodes and includes only page/pin-internal visible edges.
5. The server flushes exactly one versioned response for each input line.

### Optional vector build and search

1. Vector build resolves Arcana `CURRENT` and opens its repository snapshot.
2. Eligible facts become deterministic graph documents; valid content-addressed cache objects are reused and only misses are embedded.
3. A temporary index is written, validated, and published only while `CURRENT` still names the same graph generation.
4. Search pins and validates the matching index, embeds the query, scores records deterministically, and rechecks `CURRENT`.
5. Callers use returned stable node keys as semantic entry points into exact graph operations.

## Invariants

1. **Cross-snapshot identity is not dense identity.** Lexicon SHA-256 identity and Arcana `NodeKey` cross boundaries; `NodeId` is deterministic but snapshot-local.
2. **A packed base is immutable.** New generations create or copy artifacts; writers do not modify published packed bytes in place.
3. **An overlay is edge-only and base-bound.** Its operations apply to one exact packed base and cannot alter the dense node set.
4. **Visible graph identity is verified.** Counts, checksums, and derived IDs cover the base-plus-overlay view; compaction must preserve visible count and checksum.
5. **Repository metadata and topology form one generation.** `repository.manifest` binds and validates graph, catalogue, facts, and unresolved records.
6. **Readers observe one published generation.** Managed graph publication replaces `CURRENT` only after generation assembly; protocol pins one opened snapshot; vector work checks `CURRENT` for races.
7. **Exact graph behavior is embedding-independent.** Vectors can supply entry points but do not create, replace, or reinterpret graph relationships.
8. **Degradation is explicit.** Accepted Lexicon vocabulary loss produces persisted warnings; structural corruption and unsafe semantic guesses fail or are skipped according to the documented boundary.

## Code map

| Architecture boundary | Primary implementation | Related tests |
| --- | --- | --- |
| Library and executable boundaries | `src/lib.rs`, `src/main.rs`, `src/cli.rs` | CLI tests |
| Lexicon ingestion | `src/lexicon/` | Lexicon module tests and sync tests |
| Repository facts and dense compilation | `src/repository/` | repository module tests |
| Packed graph format and reader/writer | `src/storage/` | storage round-trip and corruption tests |
| Graph manifests, overlays, and compaction | `src/snapshot/` | graph, overlay, manifest, and compaction tests |
| Repository snapshot publication | repository snapshot modules under `src/repository/` | repository snapshot tests |
| Deterministic query protocol | `src/protocol/` | `src/protocol/tests.rs` |
| Optional vectors | `src/vector/` | vector index and document tests |
| Synthetic workloads and benchmarks | `src/synthetic/`, `src/benchmark/` | module-local tests |

Arcana does not own language adapters, higher-level evidence/workflow assembly, or the embedding service process.

## Tests

Architecture invariants are protected by repository compiler and snapshot tests, packed-storage round-trip and corruption tests, overlay and compaction tests, protocol tests, Lexicon ingestion tests, vector tests, and CLI synchronization/update tests.

## Related docs

- [Application and operations](APPLICATION.md)
- [Lexicon ingestion contract](LEXICON_CONTRACT.md)
- [Repository snapshots](repository-snapshots.md)
- [Maintainer map](MAINTAINER_MAP.md)

## Notes

Exact graph behavior remains embedding-independent; optional vectors supply entry points rather than graph truth.
