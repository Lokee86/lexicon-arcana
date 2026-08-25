# Arcana current implementation status and limitations

Parent index: [Arcana Documentation](README.md)

## Purpose

This document records Arcana's current implementation status, supported capabilities, responsibility boundaries, explicit limitations, and non-implemented possibilities.

## Overview

Status distinguishes deterministic graph functionality, optional vector behavior, standalone operation, Grimoire integration, and future ideas that are not current commitments.

This document summarizes behavior implemented in the current Arcana source and covered by focused tests. It is a status boundary, not a roadmap, performance guarantee, or substitute for the focused contracts linked below.

Detailed references:

- [Application and operations](APPLICATION.md)
- [Implementation architecture](ARCHITECTURE.md)
- [Maintainer map](MAINTAINER_MAP.md)
- [Development and verification](DEVELOPMENT.md)
- [Lexicon ingestion contract](LEXICON_CONTRACT.md)
- [Repository snapshot contract](repository-snapshots.md)
- [Semantic vector-index contract](vector-index.md)

## Status at a glance

Arcana is an independently buildable Rust library and process that consumes language-neutral repository facts and publishes immutable, validated graph generations. It currently provides deterministic packed storage, edge-only overlays, standalone and Lexicon-managed ingestion paths, an exact JSONL query protocol, bounded graph export, an optional semantic-vector index, deterministic synthetic graph generation, and a release-mode storage benchmark harness.

Arcana does not parse source languages, own language adapters, rank Grimoire's heterogeneous discovery lanes, or replace exact graph relationships with embedding similarity. Source co-location with Lexicon and Grimoire does not collapse those ownership boundaries.

Evidence: [`Cargo.toml`](../Cargo.toml), [`src/lib.rs`](../src/lib.rs), [`src/cli.rs`](../src/cli.rs), [ARCHITECTURE.md](ARCHITECTURE.md), and the [analysis-stack ownership summary](../../docs/architecture/analysis-stack.md#ownership-summary).

## Implemented capabilities

### Packed storage

The packed graph format is implemented as immutable, versioned little-endian bytes with forward and reverse offset, endpoint, and edge-kind sections. The writer canonicalizes logical edges, writes through a synchronized temporary file, and refuses to replace an existing packed path. Opening reads the file into a shared byte buffer and validates the header, exact layout and length, payload and logical-dataset checksums, offset tables, endpoint bounds, and adjacency ordering before exposing forward and reverse iterators.

The packed file stores dense topology only. Durable external identities, paths, names, kinds, content identities, source spans, facts, and unresolved records are repository metadata bound separately by `repository.manifest`.

Evidence: sibling `arcana-graph/src/storage/` owns the format, reader/writer, oracle, round-trip tests, and corruption rejection; Arcana re-exports that API through [`storage.rs`](../src/storage.rs).

### Graph and repository snapshots

A graph snapshot composes one validated packed base with zero or one overlay under `graph.manifest`. A repository snapshot binds that graph manifest to `catalogue.tsv`, `unresolved.tsv`, and `facts.tsv` under an immutable `repository.manifest`. Repository opening verifies component checksums and cross-artifact consistency before exposing data; full publication writes the repository manifest last.

Published generations are not edited in place. Managed `sync` builds under a temporary directory, renames the verified generation into its digest path, and atomically replaces `.arcana/CURRENT`. Protocol sessions open and pin one repository generation at startup.

Library-level compaction is implemented: it materializes the visible graph into a new packed base, verifies edge-count and dataset-checksum equivalence, and publishes a new base-only graph manifest without modifying the source snapshot.

Evidence: sibling `arcana-graph/src/snapshot/`, [`repository/repository_snapshot.rs`](../src/repository/repository_snapshot.rs), [`cli_sync.rs`](../src/cli_sync.rs), and focused shared-graph, repository-snapshot, sync, and compaction tests.

### Overlays and incremental updates

Overlays store canonical edge additions and removed-edge tombstones bound to an exact packed base identity. Visible forward and reverse reads merge the base with overlay operations. Incremental generations keep the original dense node mapping and write one cumulative overlay relative to the original packed base.

`update-facts` performs declared file-owner replacement against a complete replacement fact file and returns a rebuild-required error when the stable node-key-to-dense-ID map changes. Managed `sync` derives changed paths from verified Lexicon object identities; it uses an overlay when planning succeeds and otherwise rebuilds the packed base. Language-level shared-object changes force rebuilds.

Evidence: sibling `arcana-graph/src/snapshot/overlay_writer.rs` and `overlay.rs`, [`repository/incremental.rs`](../src/repository/incremental.rs), [`cli_update.rs`](../src/cli_update.rs), [`cli_sync.rs`](../src/cli_sync.rs), and their focused overlay, incremental, update, and sync tests.

### Ingestion and repository compilation

The primary integration path consumes Lexicon's immutable snapshot store. Arcana resolves and verifies Lexicon `CURRENT`, its content-addressed manifest, and every referenced binary-v1 or legacy canonical-JSON fact object, then converts normalized nodes, relationships, and unresolved records into `RepositoryFacts`. Arcana also retains complete canonical TSV import/update paths and a complete Lexicon JSONL migration/diagnostic importer.

Compilation validates nodes and relationship endpoints, orders stable `NodeKey` identities, assigns deterministic snapshot-local dense `NodeId` values, maps known relationships to stable nonzero edge codes, deduplicates repeated relationship occurrences for reachability, and emits the packed dataset plus catalogue and unresolved metadata.

Compatibility degradation is explicit: unknown node kinds become `symbol`; records with unknown edge or unresolved-relation labels are skipped; unknown unresolved-reason labels are preserved. Sync prints and persists deduplicated warnings. Malformed required data, invalid identities or paths, collisions, checksum failures, and inconsistent artifacts remain hard errors.

Evidence: [`lexicon/snapshot.rs`](../src/lexicon/snapshot.rs), [`lexicon/records.rs`](../src/lexicon/records.rs), [`repository/compiler.rs`](../src/repository/compiler.rs), Lexicon and compiler tests, and [LEXICON_CONTRACT.md](LEXICON_CONTRACT.md).

### Exact protocol operations

`arcana protocol` opens one validated repository snapshot and serves repeated JSON Lines requests over stdin/stdout. Responses use `arcana.query.v1`, echo parseable request IDs, and return either a result or a structured error. Invalid requests do not terminate the already-open session.

The current request enum and router implement 16 operations:

```text
search_nodes, resolve_symbol, resolve_file, list_nodes, export_graph,
neighbors, paths, reachability, impact, shortest_call_chain, dead_symbols,
operational_role, architecture_summary, unresolved, stats, diff
```

These operations cover exact node lookup, bounded traversal and path analysis, impact and reachability, call chains, dead-symbol and role summaries, architecture communities, unresolved evidence, graph statistics, snapshot comparison, and export. They query the visible graph, including an overlay when present.

Evidence: [`protocol/request.rs`](../src/protocol/request.rs), [`protocol/session.rs`](../src/protocol/session.rs), the narrow query modules under [`protocol/`](../src/protocol/), and [`protocol/tests.rs`](../src/protocol/tests.rs).

### Graph export

`export_graph` is a bounded protocol operation, not a packed-storage dump. It optionally normalizes a path prefix, pages matching catalogue nodes, appends validated pinned nodes without advancing the page, and emits only visible edges whose endpoints are both in the returned node set. Node and edge output is deterministic. The default page limit is 1,000 and the implementation caps a requested page at 100,000 nodes.

Evidence: [`protocol/graph_export.rs`](../src/protocol/graph_export.rs) and the paging, pinning, path-normalization, determinism, and visible-overlay cases in [`protocol/tests.rs`](../src/protocol/tests.rs).

### Optional semantic vectors

`vectorize` and the reusable `vector` module implement deterministic graph-document rendering, content-addressed cache reuse, resumable bounded-concurrency embedding, index publication, validation, and rollback. `semantic-query` validates the current or expected snapshot and embedding identity, embeds the query, exact-scans normalized `f32` records, and sorts by descending score with stable node key as the tie-breaker.

The current source uses vector-index format version 3 and semantic-eligibility policy version 6. Policy v6 indexes selected declaration, file, structural, interstack, process, CLI, protocol, and state-path entry points; it excludes high-volume detail such as variables, parameters, fields, imports, exports, constants, tests, directories, repository roots, synthetic `@...` paths, and anonymous closures/lambdas. Documents include bounded immediate graph evidence: at most 12 outgoing relationships, 12 incoming relationships, 8 unresolved records, and 6,000 UTF-8 bytes.

Vector state is optional and explicitly built. Ordinary sync, snapshot opening, packed traversal, and exact protocol operations do not require embeddings. Semantic hits are candidate entry points; exact graph operations remain authoritative for identities, relationships, paths, impact, and call chains.

Evidence: [`vector/documents.rs`](../src/vector/documents.rs), [`vector/cache.rs`](../src/vector/cache.rs), [`vector/build.rs`](../src/vector/build.rs), [`vector/index.rs`](../src/vector/index.rs), [`vector/search.rs`](../src/vector/search.rs), [`vector/documents_tests.rs`](../src/vector/documents_tests.rs), [`vector/index_tests.rs`](../src/vector/index_tests.rs), and [vector-index.md](vector-index.md).

### Synthetic workloads

Arcana implements deterministic modular, entangled, hub-heavy, layered, and dense-subsystem generators. Standard tiers range from 10,000 nodes/100,000 edges to 5,000,000 nodes/50,000,000 edges. Generation validates requested capacity and topology parameters and produces canonical, unique, directed non-self edges for a given specification and seed.

Mutation planning implements single-node, local-range, scattered, hub-focused, and one-percent edge-replacement scenarios. Mutations preserve node and edge totals and edge-kind counts while supplying the same logical visible dataset to overlay and rebuilt-packed paths.

These workloads are storage/snapshot test and benchmark inputs. They are not Lexicon ingestion, production repository samples, or evidence that every real repository has one of these topologies.

Evidence: [`synthetic/spec.rs`](../src/synthetic/spec.rs), [`synthetic/generator.rs`](../src/synthetic/generator.rs), [`synthetic/mutation/`](../src/synthetic/mutation/), their focused tests, and [`benchmark/mutation_plan.rs`](../src/benchmark/mutation_plan.rs).

### Benchmarks and evaluation evidence

`arcana benchmark` is an implemented release-mode CLI harness, not a Cargo `#[bench]` target. It generates one synthetic base, applies the five standard mutation plans, alternates overlay/rebuild measurement order, verifies visible checksum equivalence, measures validated reopen, and compares warm random, sequential, and hot-node forward/reverse query workloads using shared operations and fingerprints. Human output reports medians, speedups, throughput, and file sizes; optional CSV contains raw samples.

The harness benchmarks synthetic overlay and rebuilt-packed representations, not `.arcana/CURRENT`, repository compilation, Lexicon analysis, Grimoire discovery, or production end-to-end latency. Its focused tests prove option coverage, cleanup, and logical equivalence; test duration is not benchmark evidence. No numeric storage result is a permanent product claim: a result applies only to its exact tier, topology, seed, query/sample counts, revision, release build, toolchain, operating system, hardware, and ambient load.

The repository also retains semantic retrieval evaluations under [`evaluation/results/`](../../evaluation/results/). The paired July 2026 reports measured policy v5 on specific Grimoire and Space Rocks corpora; current source uses policy v6. Those reports are historical calibration evidence for their recorded snapshots and conditions, not a current quality guarantee or evidence of general recall.

Evidence: [`benchmark/mutation_runner.rs`](../src/benchmark/mutation_runner.rs), [`benchmark/mutation_query.rs`](../src/benchmark/mutation_query.rs), [`benchmark/report.rs`](../src/benchmark/report.rs), benchmark tests, [DEVELOPMENT.md](DEVELOPMENT.md#correctness-tests-versus-performance-evidence), and the [semantic calibration summary](../../evaluation/results/arcana-semantic-vector-calibration-2026-07-26.md).

## Standalone Arcana and Grimoire responsibilities

### Arcana owns

- Lexicon snapshot verification and conversion into language-neutral repository facts.
- Repository compilation, packed topology, graph/repository manifests, overlays, compaction, and exact graph operations.
- Optional graph-document rendering, vector cache/index state, semantic scoring, and snapshot/model/policy validation.
- Direct CLI/library operation, including standalone `import-facts` and `update-facts` outputs and managed `sync` state.

### Grimoire owns

- Prepared source and document retrieval, provider freshness/alignment, discovery sessions, cross-provider routing, result normalization, and final discovery evidence.
- Invoking Arcana as a process, matching Arcana state to the Lexicon snapshot used by a query, sending `arcana.query.v1` requests, and converting results into provider-neutral structural evidence.
- The embedding service runtime used by Arcana; Arcana's client calls the configured endpoint and does not install or host a second model.

The boundary is process- and snapshot-based. There is no Go FFI/cgo link to Arcana, and Grimoire does not read or mutate packed Arcana bytes as an internal shortcut. Grimoire's Arcana provider includes optional retrieval from an already-built semantic index and evaluation wiring exercises it, but Grimoire does not build Arcana vector state during a query. Current ordinary discovery must not be assumed to use semantic graph vectors merely because an Arcana index exists.

Evidence: [analysis-stack.md](../../docs/architecture/analysis-stack.md), [`internal/arcanagraph/README.md`](../../internal/arcanagraph/README.md), [`internal/arcanagraph/client.go`](../../internal/arcanagraph/client.go), [`internal/arcanagraph/semantic.go`](../../internal/arcanagraph/semantic.go), and [`internal/arcanaevaluation/README.md`](../../internal/arcanaevaluation/README.md).

## Current limitations and non-claims

- **No language parsing or adapter ownership.** Arcana consumes Lexicon facts; it does not discover files, parse languages, run adapters, or create authoritative source semantics.
- **Dense IDs are not durable identities.** `NodeId` values are snapshot-local. Lexicon SHA-256 identity and Arcana `NodeKey` are the cross-snapshot seams.
- **Overlays are edge-only and singular.** A graph manifest references at most one cumulative overlay. Overlays cannot add, remove, or rename nodes; standalone update fails and managed sync rebuilds when the dense mapping changes.
- **Changed-file input is not yet a partial fact batch.** `update-facts` selects changed ownership partitions from a complete replacement fact file.
- **Direct `query` is not the repository protocol.** It opens explicit packed graph and catalogue files and therefore does not apply an overlay. Use `protocol` for validated, overlay-aware queries.
- **Protocol sessions are fixed-snapshot stdio sessions.** The implementation is JSONL over stdin/stdout, not HTTP, a resident network service, or an automatically refreshing session. Restart against another generation to observe a newly published snapshot.
- **Traversal and export are bounded.** Protocol limits can truncate results. Graph export returns an induced subgraph over one page plus pins, not a complete unbounded graph dump; edges to nodes outside that returned set are omitted.
- **Unresolved evidence is retained, not resolved.** Arcana stores and queries unresolved records but has no implemented resolver pass that rewrites them into edges.
- **Compaction has no CLI command.** `compact_snapshot` is a tested library operation; `arcana compact` is not part of the current command parser.
- **Compatibility degradation can reduce graph completeness.** Unknown relationship labels are skipped with persistent warnings rather than assigned invented semantics.
- **Packed reads are not memory-mapped.** The current safe reader loads packed bytes into memory. Arcana makes no claim of lazy sharding or a resident graph service for arbitrarily large repositories.
- **Semantic search is optional, selective, and exact-scan.** It indexes only policy-eligible entry points, depends on an external plain-HTTP embedding endpoint, scans stored `f32` vectors rather than using an ANN index, and does not establish relationship truth.
- **Vector absence is not graph staleness.** Missing, stale, corrupt, or unavailable vector state blocks or degrades semantic retrieval only; it does not invalidate deterministic synchronization or exact protocol queries.
- **Synthetic scale is not production scalability proof.** Supported generator tiers and passing correctness tests do not establish latency, memory, or throughput guarantees for real repositories.
- **Benchmarks are conditional evidence.** The storage harness compares two representations under synthetic workloads. Historical semantic evaluations are corpus- and policy-specific. Neither is an SLA, a universal ranking claim, or proof that Arcana improves every Grimoire investigation.

## Future possibilities that are not implemented behavior

Focused documents mention or leave room for direct adapter-produced file-scoped fact batches, a provenance sidecar, and later unresolved-reference resolver passes. The source does not currently implement those boundaries. A compaction CLI, automatic vector construction during sync, automatic use of semantic graph vectors in every Grimoire query, approximate-nearest-neighbour indexing, and a network protocol service are likewise not current capabilities or commitments.

Treat any such change as future work until it has an owning source path, focused tests, and updated contracts. Current source constants and request enums remain authoritative when older focused documents or evaluation reports describe an earlier policy or integration state.

## Related docs

- [Application and operations](APPLICATION.md)
- [Arcana architecture](ARCHITECTURE.md)
- [Repository snapshots](repository-snapshots.md)
- [Vector index](vector-index.md)

## Notes

Update this page when an implemented capability, limit, format policy, or responsibility boundary changes.
