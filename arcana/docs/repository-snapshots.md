# Repository snapshots and incremental updates

Parent index: [Arcana Documentation](README.md)

## Purpose

This document defines Arcana repository generations, initial import, changed-file updates, immutable publication, overlay behavior, and the node-set rebuild boundary.

## Overview

A repository snapshot binds a verified graph and canonical `repository.arcana` metadata store into one immutable generation. Edge-only changes may use overlays; node-set changes require a packed rebuild.

Arcana repository snapshots bind the graph and its semantic metadata into one verified generation.

### Binary repository store v1

`repository.arcana` v1 is the canonical repository metadata store. Its frozen on-disk contract lives in `src/repository_store/format.rs`; published repository manifests are version 2 and bind this store directly. There is no migration machinery for unreleased binary drafts.

`repository.arcana` uses a fixed 512-byte little-endian header and eight 8-byte-aligned sections in this order: string table, nodes, edge facts, unresolved references, file ownership, name index, path index, and kind index. Every section records its offset, byte length, record count, and SHA-256 checksum; the header also records a SHA-256 checksum for the complete post-header payload.

The semantic invariants are:

- string IDs are deterministic lexical UTF-8 IDs, with `u32::MAX` reserved as the absent-string sentinel;
- each unique node key occupies one fixed-width node record whose row is its dense node ID; exact duplicate node facts retain multiplicity through an occurrence count;
- optional Lexicon external identities are stored as raw 32-byte SHA-256 digests rather than hexadecimal strings;
- edge records retain canonical occurrence-level facts in existing `EdgeFact::Ord` order and are not collapsed to graph edges;
- unresolved records retain existing canonical ordering, including unknown reason text through the string table;
- file ownership maps source paths to tagged node/edge/unresolved record references so later incremental replacement can read only changed-path contributions;
- name and path indexes sort dense node IDs by the referenced UTF-8 string and then ID; kind indexes use stable numeric kind codes and then ID;
- relation codes reuse the existing stable Arcana graph relation codes.

The compact value layer is implemented under `src/repository_store/`: repeated strings are interned once into the lexical string table, optional string fields use the reserved sentinel instead of heap-side option objects, source spans carry string IDs plus fixed-width coordinates, and external SHA-256 identities convert losslessly between canonical text and raw 32-byte storage. Node and unresolved codecs round-trip the current repository metadata exactly before any publication code depends on them.

`write_repository_store` emits the complete v1 file directly to disk without assembling a repository-sized serialized buffer. It canonicalizes through borrowed fact references, builds the lexical string table from borrowed `&str` values rather than cloning repository metadata strings, retains exact node multiplicity and occurrence-level edge/unresolved facts, writes file ownership against canonical record IDs, hashes each section and the complete post-header payload incrementally, then patches the fixed header. Snapshot publication now writes this file as the sole canonical metadata artifact.

`RepositoryStore` provides the corresponding lazy binary reader. Opening a store retains one immutable compact byte backing and verifies the frozen header, file length, section/payload checksums, and deterministic padding without reconstructing `RepositoryFacts` or `RepositoryCatalogue`. Node, edge, unresolved, string, ownership, and index records are decoded only when touched; string metadata is borrowed directly from the backing bytes. High-level key/name/path/path-prefix/kind queries operate on the persisted indexes, while explicit `materialize` methods remain available for compatibility boundaries that require owned facts.

A published repository snapshot contains:

- `graph.arcana` — immutable packed adjacency base;
- optional `overlay.arcana` — cumulative edge additions and tombstones relative to the packed base;
- `graph.manifest` — graph counts, checksums, and component paths;
- `repository.arcana` — canonical repository metadata, occurrence facts, unresolved evidence, ownership, and persisted indexes;
- `repository.manifest` — manifest v2 binding the graph manifest and repository store, adapter identity/version, repository identity, counts, store format version, and artifact checksums.

Opening `repository.manifest` verifies the graph-manifest checksum and the manifest-bound `repository.arcana` checksum, then opens the binary store so its header, sections, payload checksum, and padding are independently validated. Managed build/update publication no longer materializes a second catalogue or unresolved collection and does not reopen the just-written store into a repository-sized byte buffer: the writer returns a lightweight node-key checksum/count summary that is validated against the graph-only compile. `RepositorySnapshot::open` retains explicit rich-data materialization and semantic audit for vector and other rich-data consumers; interactive protocol queries use `RepositoryQuerySnapshot`.

## Hermes Phase 8 evidence

The 2026-09-27 Hermes verification confirms that the binary publication format is materially smaller while preserving graph semantics. On the same 1,136,365-node / 2,401,702-edge generation, `graph.arcana` is byte-identical at 47,002,416 bytes, while total published state falls from 1,214,675,796 bytes in the legacy TSV layout to 541,201,469 bytes with `repository.arcana` (55.44% smaller).

Runtime memory is a separate result: a current Hermes rebuild measured roughly 1.59 GB peak RSS, while managed overlay samples measured roughly 2.0–2.22 GB. The overlay path no longer reconstructs previous `RepositoryFacts`, but the safe binary reader still retains the complete ~490 MB repository-store byte backing while the complete current Lexicon facts are materialized. Storage restoration is therefore verified; managed-sync peak-memory restoration remains incomplete.

Full methodology, exact artifact sizes, parity checks, and raw result links are recorded in [Hermes Arcana storage and sync performance — 2026-09-27](../evaluation/results/hermes-arcana-storage-performance-2026-09-27/report.md).

## Initial import

```text
arcana import-facts \
  --facts repository.tsv \
  --output .arcana/generation-1 \
  --adapter go \
  --adapter-version 1
```

The output directory must not already exist. `repository.manifest` is written last.

## Changed-file update

```text
arcana update-facts \
  --base .arcana/generation-1/repository.manifest \
  --facts rescanned-repository.tsv \
  --changed internal/example/a.go \
  --changed internal/example/b.go \
  --output .arcana/generation-2
```

The standalone `update-facts` compatibility command still applies declared-path replacement semantics to its complete TSV inputs. Managed Lexicon `sync` uses the binary path instead: the prior `repository.arcana` ownership index exposes only changed-path node identities, so the previous generation is never reconstructed as a full fact set before planning the cumulative overlay.

The replacement input is currently a complete adapter fact file. Only facts owned by `--changed` paths are selected from it. This keeps the storage/update boundary ready for adapters that later emit file-scoped fact batches directly.

## Rebuild boundary

Packed node IDs are dense and immutable within a base generation. An overlay can add and remove edges, but it cannot add or remove nodes without changing those IDs.

`update-facts` therefore succeeds when the stable node-key set is unchanged. If declarations are added, removed, or renamed, Arcana returns an explicit rebuild-required error. A later generation should then be produced with `import-facts` or compaction/rebuild tooling.

This rule preserves fast packed traversal and prevents an incremental update from silently invalidating node identities used by consumers.

## Read-only query generations

`RepositoryQuerySnapshot::open` is the canonical interactive-query boundary. It opens the visible graph and a file-backed `RepositoryStoreFile`, checks manifest/generation identity, graph and metadata counts, and artifact integrity, and never reconstructs facts or invokes the repository compiler. Store validation computes the manifest checksum, section SHA-256 checksums, and payload SHA-256 in one file pass. Publication remains responsible for semantic graph/store consistency; `RepositorySnapshot::open` remains the explicit full reconstruction/audit interface for rich-data consumers.

Exact names, paths, path prefixes, kinds, and node keys use persisted indexes. Record/string reads use a bounded 4 MiB page cache. Full-text search scans strings and compact node records, ranks by canonical lexical string IDs, and decodes only retained response nodes. Its per-query rank scratch has an 8 MiB memory budget and spills larger tables to an anonymous temporary file cleaned up when the handle closes, including process termination. It does not change published snapshot files. Unresolved queries merge canonical source ordering with dense node keys and decode only returned references; adjacent duplicate records collapse to the existing protocol semantics. Statistics read compact codes and the visible graph. Diff opens both generations through the same query owner and retains bounded response values.

The query protocol preserves `arcana.query.v1` ordering, filtering, pagination, errors, and generation-local dense IDs. It owns no reconstructed catalogue, global unresolved vector, or source-reference map. The obsolete rich-snapshot protocol extraction method has been removed.

## Code map

| Snapshot concern | Primary implementation | Related tests |
| --- | --- | --- |
| Repository manifest and publication | `src/repository/repository_snapshot.rs`, `repository_snapshot_format.rs`, `repository_snapshot_validation.rs` | `repository_snapshot_tests.rs` |
| Packed graph manifest | `src/snapshot/graph.rs`, `manifest.rs`, `manifest_io.rs` | graph and manifest tests |
| Overlay format and visible reads | `src/snapshot/overlay_*.rs`, `overlay.rs` | overlay and graph tests |
| Initial import | `src/cli_commands.rs`, repository compiler, storage writer | CLI, repository, and storage tests |
| Changed-file update | `src/cli_update.rs`, `src/repository/ownership.rs`, `incremental.rs` | update, ownership, and incremental tests |
| Managed Lexicon synchronization | `src/cli_sync.rs`, `src/cli_sync_state.rs`, `src/cli_sync_build.rs`, `src/repository/incremental_store.rs`, `src/repository_store/reader_incremental.rs` | sync, ownership-index, and store-backed incremental tests |
| Compaction | `src/snapshot/compaction.rs` | compaction tests |

Overlays may change edges only. Node-set changes require a packed rebuild.

## Related docs

- [Arcana architecture](ARCHITECTURE.md)
- [Application and operations](APPLICATION.md)
- [Lexicon ingestion contract](LEXICON_CONTRACT.md)
- [Current status](STATUS.md)

## Notes

Dense node identifiers are generation-local and may not be silently reinterpreted across a node-set change.
