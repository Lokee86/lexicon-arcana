# Arcana reference

Parent index: [Reference](INDEX.md)

## Purpose

This document defines Arcana's product-facing commands, state, synchronization, graph, protocol, vector, degradation, and diagnostic contracts inside the Grimoire bundle.

## Overview

Arcana consumes an immutable Lexicon snapshot and publishes verified graph state for deterministic structural queries. Grimoire normally coordinates it automatically, while direct commands remain available for specialist operation and development.

Arcana is the repository-graph component in the Grimoire bundle. It consumes one immutable Lexicon snapshot and publishes a verified graph snapshot optimized for deterministic traversal, impact analysis, paths, call chains, architecture summaries, and structural inspection.

Grimoire normally synchronizes and queries Arcana automatically. Direct Arcana commands are for graph operators, protocol integrators, storage developers, benchmark work, and semantic-index diagnostics.

## Command access

Use either the standalone executable:

```text
arcana <command> ...
```

or Grimoire's forwarding namespace:

```text
grimoire arcana <command> ...
```

`grimoire arcana check` reports the resolved executable and version. Other arguments and process streams are forwarded unchanged.

## Core operations

| Operation | Purpose |
| --- | --- |
| `import-facts` | Compile one complete canonical TSV fact file into a new standalone repository snapshot directory. |
| `update-facts` | Replace facts owned by declared changed paths and publish a new standalone snapshot with a cumulative edge overlay when node identities remain stable. |
| `sync` | Verify a Lexicon snapshot and publish or reuse the corresponding managed Arcana graph snapshot. |
| `sync --register` | Register Arcana as a Lexicon post-publication consumer. |
| `protocol --snapshot <path>` | Serve line-oriented `arcana.query.v1` requests against one immutable, overlay-aware repository snapshot. |
| `query` | Perform direct lookup against explicitly supplied packed graph and catalogue files; this path does not apply repository overlays. |
| `vectorize` | Explicitly build the optional semantic graph index for the current graph snapshot. |
| `semantic-query` | Find graph entry points through the optional semantic index. |
| `benchmark` | Compare packed rebuild and immutable-overlay update/query behavior on deterministic synthetic graphs. |

Exact flags, defaults, exit behavior, standalone-versus-managed publication, and diagnostics are documented in [`arcana/docs/APPLICATION.md`](../../arcana/docs/APPLICATION.md). Command parsing is owned by `arcana/src/cli.rs`; execution is split across the command-specific CLI modules.

## State layout

Arcana publishes state beneath `.arcana/`. The current snapshot is immutable and bound to the exact Lexicon snapshot it consumed.

A graph snapshot contains:

```text
graph.arcana
catalogue.tsv
unresolved.tsv
snapshot manifest
optional overlay
```

The state root also contains writer coordination and optional semantic-vector state. `.arcana/CURRENT` advances only after the candidate graph state validates.

## Synchronization lifecycle

1. Resolve `.lexicon/CURRENT` or an explicitly selected Lexicon snapshot.
2. Validate the Lexicon manifest and each referenced fact object.
3. Decode normalized nodes, relationships, and unresolved evidence.
4. Build the dense node catalogue and repository fact model.
5. Choose a packed-base rebuild or an immutable overlay update.
6. Validate forward and reverse graph state, counts, ordering, and checksums.
7. Publish the Arcana snapshot and atomically advance `.arcana/CURRENT`.

If the graph node set remains stable, relationship-only changes may use an overlay. Symbol additions/removals or shared language-level fact changes rebuild the packed base.

## Packed graph

The packed graph stores aligned forward and reverse adjacency sections in a versioned little-endian format. Readers query the packed bytes directly rather than reconstructing a complete in-memory graph.

Opening state validates:

- header and section layout;
- file length and checksums;
- node bounds and offset tables;
- canonical adjacency ordering;
- logical dataset identity.

An in-memory graph implementation remains the correctness oracle for tests.

## Snapshots, overlays, and compaction

An Arcana snapshot is a composition of one validated packed base and an optional immutable overlay. The overlay stores added edges and removed-edge tombstones bound to the exact base identity.

Forward and reverse queries merge base adjacency with overlay indexes. Compaction writes a new packed base for the visible graph and publishes a new base-only manifest. It does not mutate the source snapshot.

## Query protocol

The stable protocol identifier is `arcana.query.v1`. Consumers begin with `capabilities`, which returns protocol version `1`, the Arcana implementation version, and the supported operation names. Consumers must reject unsupported protocol versions or missing required operations rather than inferring compatibility from response shape.

Implemented operations include:

- capability and operation discovery;
- symbol and file resolution;
- forward and reverse neighbors;
- bounded multi-hop paths;
- entry-point reachability;
- transitive impact;
- shortest call chains;
- unresolved references;
- graph statistics;
- snapshot differences;
- dead-symbol detection;
- operational-role summaries;
- deterministic architecture communities and summaries;
- graph export.

Architecture summaries can be scoped by path and relationship kind and report representative nodes plus internal and boundary relationship counts.

Grimoire communicates through this protocol from `internal/arcanagraph/`; it does not read Arcana's packed format as an internal implementation shortcut.

## Optional semantic graph index

Arcana can explicitly vectorize eligible declaration-level graph entry points and bounded neighborhoods using Grimoire's configured OpenAI-compatible embedding endpoint.

The semantic index is optional:

- ordinary synchronization does not build it;
- deterministic graph traversal does not require it;
- semantic matches identify entry points, while exact Arcana traversal remains authoritative;
- cached graph-document vectors can be reused across snapshots when rendered content is byte-identical.

See [`arcana/docs/vector-index.md`](../../arcana/docs/vector-index.md) for storage, invalidation, resume, and query behavior.

## Lexicon boundary

Lexicon owns language parsing and durable normalized fact identities. Arcana owns graph compaction and snapshot-local dense IDs.

Arcana records the consumed Lexicon snapshot ID and rejects incompatible or corrupted input. It may read legacy fact encodings during migration, but the current integration boundary is the immutable Lexicon snapshot/object contract.

See [`arcana/docs/LEXICON_CONTRACT.md`](../../arcana/docs/LEXICON_CONTRACT.md).

## Degradation in Grimoire

When Arcana is unavailable or stale, Grimoire can still return exact source, BM25 source, document, and Lexicon symbol evidence. Relationship evidence may fall back to direct Lexicon facts with reduced traversal capability.

Arcana vector state is independent. Missing graph vectors do not make deterministic graph state stale.

## Common diagnostics

### No graph snapshot

Confirm Lexicon has a valid current snapshot, then run Arcana synchronization. Arcana cannot create authoritative language facts itself.

### Snapshot rejected

A manifest/object checksum mismatch, catalogue collision, invalid packed layout, or Lexicon identity mismatch is a hard error. Rebuild from verified Lexicon state rather than editing Arcana files manually.

### Arcana does not follow Lexicon scans

Run `sync --register` and inspect the Lexicon consumer definition/state. The event-driven path is a bounded consumer invocation, not a resident Arcana daemon.

### Queries return no symbol

Inspect the catalogue and confirm the symbol exists in the consumed Lexicon snapshot. Name resolution is constrained by actual graph catalogue entries and protocol query fields.

### Semantic query unavailable

The optional index must be built explicitly and must match the current graph and embedding identities. Deterministic protocol queries remain available without it.

## Code map

| Documented concern | Primary implementation | Related tests |
| --- | --- | --- |
| Grimoire process/protocol integration | `internal/arcanagraph/` | `internal/arcanagraph/*_test.go` |
| Arcana command surface | `arcana/src/cli.rs`, `arcana/src/cli_*.rs`, `arcana/src/main.rs` | Arcana CLI tests |
| Lexicon snapshot ingestion | `arcana/src/lexicon/` | Arcana Lexicon and sync tests |
| Repository compilation and catalogue | `arcana/src/repository/` | repository module tests |
| Packed graph and topology snapshots | `arcana-graph/src/storage/`, `arcana-graph/src/snapshot/` (re-exported by Arcana) | shared storage and snapshot tests |
| Query protocol | `arcana/src/protocol/` | protocol tests |
| Optional vectors | `arcana/src/vector/` | vector tests |

Grimoire consumes Arcana through `arcana.query.v1`; it does not read packed graph bytes directly.

## Related docs

- [Arcana application](../../arcana/docs/APPLICATION.md)
- [Arcana architecture](../../arcana/docs/ARCHITECTURE.md)
- [Arcana maintainer map](../../arcana/docs/MAINTAINER_MAP.md)
- [Analysis stack](../architecture/analysis-stack.md)

## Notes

Use the subject document's code map for the narrow implementation path. Use the maintainer map only when ownership is unclear.
