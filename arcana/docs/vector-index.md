# Arcana Semantic Graph Index

Parent index: [Arcana Documentation](README.md)

## Purpose

This document defines Arcana's optional semantic graph-document policy, embedding identity, cache, index storage, build publication, query scoring, and process-integration boundary.

## Overview

The semantic index supplies bounded graph entry points for conceptual queries. Exact graph traversal, repository snapshot validity, and structural relationships remain deterministic and authoritative without vectors.

Arcana can build an optional semantic index over the current immutable repository graph. The index provides semantic entry points into Arcana's deterministic graph traversal without moving graph ownership into an embedding service or higher-level consumer.

## Ownership

- Arcana owns graph-document generation, vector persistence, index invalidation, and semantic graph search.
- The embedding model runtime and endpoint are external to Arcana; Warlock or another local service may provide them.
- Arcana requests embeddings from that endpoint; it does not install or load a second model.
- Lexicon remains the authority for language facts and source identities.

The ordinary `arcana sync`, graph protocol, and packed snapshots remain embedding-free. A missing embedding server or vector index does not affect deterministic graph construction or graph queries.

## Build the index

Start an OpenAI-compatible embedding service, synchronize Arcana, then build the semantic graph index explicitly:

```text
# start an OpenAI-compatible embedding service separately
arcana sync
arcana vectorize
```

Options:

```text
arcana vectorize \
  [--state <DIRECTORY>] \
  [--endpoint <URL>] \
  [--batch-size <N>] \
  [--batch-concurrency <N>]
```

Defaults:

- state: `.arcana`
- endpoint: `http://127.0.0.1:9876/v1`
- batch size: `32`
- batch concurrency: `1`
- model identity: `qwen3-embedding-0.6b-q8_0-512d`
- retained dimensions: `512`

The command first validates an exact existing snapshot, including its semantic-eligibility policy. Otherwise it resolves every eligible graph document to the immutable content-addressed cache, sends only missing documents to the endpoint, and materializes the snapshot files from cache objects. `--batch-concurrency` bounds simultaneous embedding requests; each successful batch writes and synchronizes its objects immediately, so a retry after an endpoint or process interruption resumes from completed work. When a provider rejects a multi-document request for exceeding its context limit, Arcana bisects that request until the provider accepts it or one individual document still exceeds the contract. Snapshot materialization remains serialized and deterministic.

Build output reports indexed-document and unique-vector counts, embedded and reused vectors, exact-snapshot reuse, embedding request count, snapshot bytes, elapsed milliseconds, and the published directory. The exact-snapshot path makes no embedding request. Cross-snapshot reuse occurs only when rendered graph-document bytes and the complete embedding contract are identical.

Build validation covers the manifest, semantic-eligibility policy, fixed data filenames, vector byte length, finite values, indexed-document record count and identities, and data checksums. Incomplete, corrupt, or policy-incompatible snapshots are rebuilt. Publication is serialized per snapshot and embedding identity, preserves the prior index if replacement fails, and aborts with a retry message rather than publishing under the wrong snapshot if `.arcana/CURRENT` changes during embedding.

## Indexed objects

Arcana constructs the complete repository graph, then applies semantic-eligibility policy version 6 only while generating index documents. The policy excludes `variable`, `parameter`, `field`, `import`, `export`, `constant`, `test`, `directory`, and `repository` nodes. It also excludes synthetic external or built-in paths beginning with `@` and anonymous closure or lambda declarations. It includes named `function`, `method`, `constructor`, `file`, `type`, `interface`, `trait`, generic `symbol`, `module`, `namespace`, `message-channel`, `http-endpoint`, `config-key`, `signal`, `process`, `cli-command`, `protocol`, and `state-path` nodes. Every current node kind is matched explicitly, so adding a kind requires a deliberate policy decision.

These exclusions remove high-volume implementation detail and low-value semantic hubs that are usually reached more precisely by traversing from a declaration-level match. They do not remove nodes or edges from Arcana snapshots, so exact traversal can still reach variables, parameters, fields, imports, exports, constants, tests, synthetic nodes, directories, and the repository root.

Arcana creates one deterministic document per eligible graph node, ordered by stable node key. Each document contains:

- node kind, exact name, exact path, source span, and deterministic natural-language terms derived from camel case, snake case, acronyms, and path separators;
- up to 12 outgoing relationships and target identities, including exact and natural-language target names, restricted to semantic entry-point nodes and ordered with calls, routes, handlers, publications, and other operational relationships first;
- up to 12 incoming relationships and source identities under the same semantic-neighbour policy; and
- up to 8 unresolved-reference records.

Rendered documents are capped at 6,000 UTF-8 bytes with a deterministic truncation marker. This prevents generated source expressions or unresolved adapter payloads from exceeding the embedding provider's per-document context while retaining node identity and the highest-priority neighborhood evidence.

The embedding finds a relevant graph neighborhood. Arcana's exact graph protocol then resolves and expands the returned nodes. Vectors never replace authoritative graph relationships.

## Storage

Reusable vector objects are stored independently from indexes, which remain keyed by Arcana snapshot and model identity:

```text
.arcana/
  vector-cache/
    qwen3-embedding-0.6b-q8_0-512d/
      objects/
        <digest-prefix>/
          <document-and-embedding-digest>.avec
  vectors/
    <arcana-snapshot-digest>/
      qwen3-embedding-0.6b-q8_0-512d/
        manifest.json
        nodes.jsonl
        vectors.f32
```

The cache key is SHA-256 over a domain separator, rendered graph-document bytes, model reference, stable embedding identity, and dimensions. Node keys are intentionally absent from rendered documents, so a node-key rename can reuse an object; a changed name, path, span, relationship neighborhood, unresolved reference, model, identity, or dimension cannot. Objects contain their content key, dimensions, vector checksum, and normalized little-endian `f32` values. Invalid objects are re-embedded and atomically repaired. Cache objects are immutable once valid.

`manifest.json` binds the index to:

- repository snapshot ID;
- graph snapshot ID;
- embedding model and stable model identity;
- semantic-eligibility policy version and a composite index identity containing that version;
- vector dimensions; and
- item count, fixed data filenames, and SHA-256 checksums of both data files.

`vectors.f32` contains normalized little-endian `f32` vectors materialized in deterministic node order. `nodes.jsonl` maps vector positions back to Arcana node keys, kinds, paths, and names. A snapshot manifest also records the unique-vector count when produced by the incremental builder.

The current index format is version 3 and semantic-eligibility policy version is 6. Its composite identity is `<embedding-identity>-arcana-semantic-v6`, while the containing directory remains keyed by embedding identity. Earlier manifests lack the current checksums, entry-point policy, operational node coverage, or bounded document-rendering contract and are rebuilt rather than reused.

## Expected scope reduction

The policy is intentionally a substantial reduction, not a graph-size change. In the measured live states that motivated it:

- Grimoire had 30,260 graph nodes, including 22,295 variables. Excluding variables alone caps the semantic index at 7,965 documents, at least a 73.7% reduction; the other excluded kinds reduce it further.
- Space Rocks had 64,069 graph nodes, including 31,218 variables and 6,516 parameters. Excluding those two kinds alone caps the semantic index at 26,335 documents, at least a 58.9% reduction; fields, imports, exports, directories, and the repository node reduce it further.

Manifest `item_count`, vector byte-length validation, record-count validation, and Arcana build summaries report and validate the indexed-document count. They do not compare that count with the complete graph node count.

## Query the index

Human-readable output:

```text
arcana semantic-query --query "where is profile persistence handled?"
```

Machine-readable output:

```text
arcana semantic-query \
  --query "where is profile persistence handled?" \
  --limit 10 \
  --json
```

The JSON response has this shape:

```json
{
  "matches": [
    {
      "score": 0.72,
      "node_key": "0123456789abcdef",
      "kind": "function",
      "path": "internal/profile/repository.go",
      "name": "InsertProfile"
    }
  ]
}
```

Semantic query performs cheap manifest and file-size checks when opening the pinned snapshot, then decodes and finite-checks each vector in the same single scoring pass. It does not checksum or pre-scan the complete vector file on every query. Full checksums and exhaustive structural validation remain build/validation boundary work.

## Process integration

Process integrations can pass `--expected-snapshot sha256:<digest>` to `semantic-query`. Arcana then rejects the query if `.arcana/CURRENT` no longer matches the graph snapshot that the caller already resolved. This prevents semantic seeds from one graph snapshot being expanded through another.

Higher-level consumers do not automatically gain or require Arcana semantic-vector behavior merely because an index exists. The semantic index remains an explicit Arcana surface for standalone CLI use and paired graph-retrieval evaluation; deterministic Lexicon facts and Arcana graph operations remain independently available.

## Code map

| Vector concern | Primary implementation | Related tests |
| --- | --- | --- |
| Eligible graph documents and policy identity | `src/vector/documents.rs` | `src/vector/documents_tests.rs` |
| Embedding client and transport | `src/vector/client.rs`, `http.rs` | inline/client tests |
| Content-addressed cache | `src/vector/cache.rs` | cache and index tests |
| Build, reuse, and publication | `src/vector/build.rs`, `index.rs` | `src/vector/index_tests.rs` |
| Query scoring | `src/vector/search.rs` | vector index/search tests |
| CLI integration | `src/cli_vectors.rs` | CLI tests |

The vector index is optional and supplies semantic entry points. Exact graph traversal and snapshot validity remain independent and authoritative.

## Related docs

- [Arcana architecture](ARCHITECTURE.md)
- [Application and operations](APPLICATION.md)
- [Repository snapshots](repository-snapshots.md)
- [Current status](STATUS.md)

## Notes

Changing semantic eligibility, document rendering, model identity, dimensions, or policy version invalidates incompatible cached or indexed state.
