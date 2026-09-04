# Arcana

> **Canonical source:** Arcana lives under `arcana/` in the shared `Lokee86/lexicon-arcana` repository. It remains an independently buildable Rust application, CLI, protocol, and reusable graph engine.

Arcana is the graph-analysis product in the Lexicon + Arcana stack and a deterministic analysis provider for the [**Warlock Toolchain**](https://github.com/Lokee86/warlock-toolchain).
It models repositories as queryable graphs and provides storage, snapshots, traversal, impact, paths, call chains, and architecture queries for humans and higher-level tools such as Warlock and Pitlord.



## Ownership boundaries

- **Lexicon** owns language parsing and the normalized symbol/relationship fact contract.
- **Arcana** owns graph ingestion, packed storage, snapshots, deterministic traversal,
  optional semantic graph indexes, and measurements of storage representations.
- **Demon Docs** owns documentation semantics, policy, review history, and
  Codemap decisions. It consumes Arcana facts without owning the graph
  engine.
- **Warlock or another consumer** owns agent/task/context orchestration and higher-level probabilistic workflow.
- **Grimoire** is retired. Its former discovery, retrieval, stable-handle, and investigation-session responsibilities are not being transferred into Arcana.


Arcana remains a standalone Rust process or CLI boundary. Go consumers do
not link it through cgo or FFI.

### Repository boundary

Arcana and Lexicon share the `lexicon-arcana` repository, but the implementation boundary remains intentional. Arcana is a separate Rust process, independently testable and directly usable for graph work. Lexicon snapshots and the Arcana protocol remain the authoritative integration boundaries; source co-location does not permit direct mutation of Lexicon state or language-analysis ownership.

## Graph workload foundation

Arcana includes deterministic synthetic graph generation for exercising
its packed storage, snapshot, overlay, and compaction systems across more than a
single toy topology.

The workload foundation currently includes five topology families:

- **Modular** — cohesive clusters with a configurable cross-cluster edge share.
- **Entangled** — hubs, cross-cluster relationships, cycles, and local edges.
- **Hub-heavy** — a small set of nodes owns most incoming and outgoing edges.
- **Layered** — deep, mostly forward relationships with a smaller irregular
  edge set.
- **Dense subsystem** — a tightly connected subsystem inside a larger sparse
  graph.

Standard scale tiers range from 10,000 nodes and 100,000 edges to 5,000,000
nodes and 50,000,000 edges. Generation scales with requested edges rather than
enumerating every possible node pair.

Mutation plans cover single-node, local-range, scattered, hub-focused, and
percentage updates. A plan contains exact removed and replacement edges so the
overlay and rebuilt packed snapshot receive the same logical update.

## Determinism and invariants

Synthetic datasets and mutations are controlled by explicit seeds. Generated
and mutated graphs guarantee:

- exact requested edge counts;
- unique directed non-self edges;
- canonical source/target/kind ordering;
- topology-specific edge distributions;
- stable output for the same specification and seed; and
- preserved edge-kind counts across mutations.

The generator uses a small internal permutation sampler and has no third-party
dependencies. Dataset construction is outside the measured update and query
windows and is reused by both benchmark paths.

## Packed adjacency format

The immutable packed format uses a fixed, versioned, little-endian header
followed by aligned forward offsets, forward targets, forward edge kinds,
reverse offsets, reverse sources, and reverse edge kinds. The writer
canonicalizes logical edges, streams deterministic bytes through a temporary
file, syncs them, and atomically commits a new snapshot path.

Opening a packed graph validates the header, exact section layout, file length,
payload checksum, logical dataset checksum, offset tables, node bounds, and
adjacency ordering. Queries read directly from the packed byte buffer without
rebuilding an in-memory graph. A separate in-memory implementation provides the
correctness oracle used by round-trip tests.

## Snapshots and overlays

Arcana snapshots are immutable compositions rather than mutable graph
files. A snapshot manifest identifies one validated packed base plus an optional
immutable overlay. The manifest is published last, so readers never observe a
partially assembled snapshot.

Overlay v1 stores canonical added edges and removed-edge tombstones bound to the
exact node count, edge count, and dataset checksum of its packed base. Opening a
snapshot validates the base, overlay, manifest identities, visible edge count,
and visible dataset checksum before queries are allowed. Forward and reverse
queries merge the base adjacency with small in-memory overlay indexes without
rewriting the packed base.

Snapshot and overlay files are content-identifiable and refuse in-place
replacement. `compact_snapshot` materializes the visible graph into a new packed
base, verifies its edge count and checksum, and publishes a new base-only
manifest last. The source snapshot remains untouched.

## Repository ingestion

Arcana consumes Lexicon's immutable snapshot store, verifies each content-addressed
fact object, and decodes Lexicon binary v1 node, edge, and unresolved sections
into typed repository facts without reconstructing JSONL. Legacy canonical JSON
fact objects remain readable during migration. Language adapters live in the co-located [`../lexicon`](../lexicon/) component; Arcana does not own language parsers.

### Lexicon synchronization

A one-shot sync reads `.lexicon/CURRENT`, verifies the referenced manifest and
fact objects, and publishes an immutable graph snapshot under `.arcana/`:

```text
lexicon scan
arcana sync
```

Run `arcana sync --register` once to add Arcana as a Lexicon post-publication
consumer. Every later successful manual or daemon-triggered Lexicon scan then
invokes the same one-shot Arcana sync command. Arcana compares successive
Lexicon file-object identities, writes an overlay when the graph node set is
stable, and rebuilds the packed base when symbols were added or removed or any
language-level shared fact object changed. Neither process must remain resident
for this event-driven path.

Each Arcana snapshot records the consumed Lexicon snapshot ID. Arcana serializes
writers through `.arcana/LOCK` and atomically replaces `.arcana/CURRENT` only
after the new graph state verifies, so manual and hook-triggered syncs cannot race
or expose a partial publication.

The compiled output contains:

- `graph.arcana` — packed forward and reverse adjacency;
- `catalogue.tsv` — dense node IDs mapped back to full Lexicon identities,
  compact stable keys, paths, names, kinds, content IDs, and source spans;
- `unresolved.tsv` — unresolved-reference evidence keyed back to catalogue nodes.

```text
arcana sync --lexicon /path/to/repository/.lexicon \
  --state /path/to/repository/.arcana

arcana protocol --snapshot \
  /path/to/repository/.arcana/snapshots/<lexicon-snapshot-digest>
{"id":"symbol","op":"resolve_symbol","name":"ExampleFunction"}
{"id":"chain","op":"shortest_call_chain","from_node_id":12,"to_node_id":42}
{"id":"impact","op":"impact","node_id":42,"max_depth":8}
{"id":"architecture","op":"architecture_summary","path_prefix":"src","min_community_size":3}
```

Arcana also reads its legacy TSV fact format during migration. Lexicon SHA-256
identities remain the durable cross-tool identity; Arcana compacts them into
snapshot-local packed IDs and rejects any detected compaction collision.

The stable protocol identifier is `arcana.query.v1`. In addition to symbol,
file, neighbor, unresolved, statistics, and snapshot-diff operations, it supports
bounded multi-hop paths, entry-point reachability, transitive impact, shortest
call chains, dead-symbol detection, operational-role summaries, and deterministic
architecture communities. Architecture summaries can be path-scoped and relation-
scoped, and report representative nodes, internal relation counts, and incoming or
outgoing boundary counts.

The repository fact model also accepts `observed-calls`, `routes-to`,
`communicates-with`, and `similar-to` evidence. Runtime-observed calls participate
in call traversal and are reported separately from static call resolution.

See [`docs/LEXICON_CONTRACT.md`](docs/LEXICON_CONTRACT.md) for the exact consumer
boundary and incremental ownership policy.

## Optional semantic graph index

Arcana can explicitly vectorize the current immutable graph through a generic
OpenAI-compatible embedding endpoint. Arcana stores and invalidates the
graph vectors; it does not install or own the embedding runtime. Warlock or another local service may provide the endpoint. Ordinary `arcana sync`
and graph-protocol operations remain embedding-free.

```text
# start an OpenAI-compatible embedding service separately
arcana sync
arcana vectorize
arcana semantic-query --query "where is profile persistence handled?"
```

The packed index lives under
`.arcana/vectors/<snapshot-digest>/<embedding-identity>/`. Immutable graph-document
vectors live under `.arcana/vector-cache/<embedding-identity>/` and are reused
across snapshots when rendered content is byte-identical. Successful concurrent
embedding batches persist immediately, so interrupted builds resume without
repeating completed work. Each vector represents an eligible declaration-level
graph entry point plus a bounded immediate neighborhood. Variables, parameters,
fields, imports, exports, directories, and repository roots remain in the complete
graph but are not indexed directly. Semantic matches provide entry points; exact
Arcana traversal remains authoritative for relationships, impact, and call chains.

The semantic index is currently consumed by Arcana's standalone `semantic-query`
command and paired Arcana evaluation. The semantic index is optional;
deterministic Lexicon facts and Arcana graph queries do not
silently build or require the semantic index.

See [`docs/vector-index.md`](docs/vector-index.md) for storage, invalidation,
commands, and integration details.

## Benchmarks

The benchmark harness compares immutable overlays against rebuilding a complete
packed replacement while treating the existing packed base as shared storage:

```text
cargo run --release -- benchmark \
  --tier small \
  --topology modular \
  --queries 10000 \
  --samples 3 \
  --csv target/benchmarks/small-modular-mutations.csv
```

Supported tiers are `small`, `medium`, `large`, and `stress`. Supported
synthetic topologies are `modular`, `entangled`, `hub-heavy`, `layered`, and
`dense-subsystem`.

The five mutation workloads cover one hot node, a local range, scattered
changes, hub-focused changes, and one percent of all edges. Each run measures
creation, fully validated reopen, warm forward/reverse queries, and incremental
file size. Overlay and rebuilt-packed results must produce identical visible
graph checksums and query fingerprints.

## Documentation

- [Documentation index](docs/README.md)
- [Application and operations](docs/APPLICATION.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Maintainer map](docs/MAINTAINER_MAP.md)
- [Development and verification](docs/DEVELOPMENT.md)
- [Current status and limitations](docs/STATUS.md)
- [Lexicon integration contract](docs/LEXICON_CONTRACT.md)
- [Repository snapshot contract](docs/repository-snapshots.md)
- [Optional semantic vector index](docs/vector-index.md)

## Development

The package uses Rust edition 2024.

```text
cargo fmt -- --check
cargo check --all-targets
cargo test --all-targets
cargo run -- --help
cargo run -- --version
```

## License

Arcana is available under the [PolyForm Shield License 1.0.0](LICENSE.md). Competing products and services require a separate commercial license. See the repository root [LICENSING.md](../LICENSING.md).
