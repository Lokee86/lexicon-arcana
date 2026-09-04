# Vector store

Parent index: [Reference](INDEX.md)

Historical status: this page describes the retired Grimoire document-vector/Lodestone integration and is retained only for prior benchmark/design interpretation. It is not an active product contract.

## Purpose

This document defines Grimoire's Lodestone-backed documentation-vector storage boundary, state identities, build and publication behavior, ABI expectations, and degradation semantics.

## Overview

Lodestone owns immutable vector objects and exact search. Grimoire owns document-vector manifests, freshness, build orchestration, ranking integration, and the Go compatibility facade.

Grimoire stores normalized documentation embeddings through Lodestone, exposed to Go by Lodestone's shared binding and Grimoire's `internal/vectorstore` compatibility facade. Arcana separately owns graph-neighbourhood vector indexes under `.arcana/`.

Repository-wide source-code embeddings are not part of production source retrieval.

## State location

For the fixed embedding identity, documentation vector state is stored below the knowledge index:

```text
<root>/.grimoire/knowledge/vectors/qwen3-embedding-0.6b-q8_0-512d/
```

The directory contains immutable objects, the current packed snapshot, and `snapshot.manifest.json`. Temporary JSONL ingestion and record-list files are removed after use.

## Layout and identities

The engine maintains:

- immutable content-addressed vector objects;
- a manifest binding knowledge-section IDs to vector source identities; and
- a sorted packed snapshot used for memory-mapped exact search.

An object address includes the storage format identity, embedding identity, and section-content hash. Identical documentation text can reuse one vector across multiple stable section IDs.

The manifest records the exact knowledge-index identity. The identity covers document and section hashes, so documentation changes make an older snapshot stale before query embedding occurs.

## Build and publication

```bash
grimoire knowledge index --root <repository>
grimoire vector build --root <repository>
```

The root vector command is an alias for `grimoire knowledge vector build`. It sends only missing documentation-section embeddings to the native `IngestJSONL` boundary. Each successful batch writes immutable objects immediately. After all required vectors exist, Grimoire writes the complete section manifest and materializes the packed snapshot.

A failed build does not publish an incomplete manifest, while objects completed before the failure remain reusable. Repeated builds reuse unchanged objects and can resume after a failed batch.

## Snapshot reads and search

`grimoire knowledge search` always performs BM25. Pass `--vectors=true` to supplement it with vector scores from a current snapshot; the default path does not embed the query.

The engine performs exact inner-product search over normalized 512-dimensional vectors. The packed format is an exact-search representation, not an approximate nearest-neighbour index.

Snapshot handles are opaque native values. The Go bridge serializes `Info`, `Search`, and `Close` operations per handle because the native handle is not re-entrant. Borrowed Go buffers are kept alive for every ABI call.

## ABI contract

- Strings cross as UTF-8 pointer-and-length pairs.
- Go owns all query, result, ID, and metadata buffers.
- Rust borrows foreign buffers only for one call and never retains Go pointers.
- Rust allocations are not returned for Go to free.
- Snapshot handles are numeric registry keys rather than raw pointers.
- Panics are converted to ABI errors.

## Compatibility and discovery

A documentation snapshot is accepted only when its manifest agrees with the current knowledge-index identity, embedding identity, dimensions, and vector count. `grimoire vector info` reports availability and freshness.

On Windows, Grimoire checks `LODESTONE_LIBRARY`, the legacy `GRIMOIRE_VECTOR_ENGINE`, the executable directory, and sibling `lodestone/target/{release,debug}` builds beneath workspace ancestors. Lodestone's Rust core is portable, but equivalent non-Windows Go loaders are not yet implemented.

Missing, stale, incompatible, or unavailable documentation vectors leave the document lane on BM25 and expose a warning. They never affect exact, source, symbol, relationship, trace, or impact discovery.

## Ownership boundary

Lodestone owns immutable objects, packed snapshots, validation, exact vector search, the stable C ABI, and the shared Go loader. `internal/vectorstore` preserves Grimoire's internal package boundary. `internal/knowledgevector` owns documentation manifests, freshness, build orchestration, and ranking integration. Embedding and BM25 ranking remain outside Lodestone.

## Code map

| Concern | Primary implementation | Related tests |
| --- | --- | --- |
| Grimoire vector-storage boundary | `internal/vectorstore/types.go` | `internal/vectorstore/integration_windows_test.go` |
| Document-vector build and state | `internal/knowledgevector/build.go`, `state.go`, `manifest.go` | knowledge-vector state and embedding tests |
| Query ranking | `internal/knowledgevector/ranker.go` | package-local tests |
| Embedding input | `internal/embedding/` | embedding package tests |
| CLI orchestration | `internal/app/knowledge_vectors.go` | `internal/app/vector_test.go`, `vector_resume_test.go` |
| Native engine implementation | sibling Lodestone repository | verified through the compatibility/integration boundary, not owned here |

Grimoire owns the compatibility boundary and publication workflow, not Lodestone's internal storage engine.

## Related docs

- [Embedding model](embedding-model.md)
- [Knowledge retrieval](knowledge.md)
- [Indexing](indexing.md)
- [Component architecture](../architecture/components.md)

## Notes

Arcana's graph-vector index is a separate component-owned facility under `.arcana/`.
