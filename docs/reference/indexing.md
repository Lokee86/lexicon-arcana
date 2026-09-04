# Indexing

Parent index: [Reference](INDEX.md)

Historical status: this page describes the retired Grimoire prepared-index layer and is retained only for prior benchmark/design interpretation. It is not an active product contract.

## Purpose

This document defines Grimoire's source preparation, documentation indexing, eligibility, exclusions, chunking, reuse, token accounting, and freshness behavior.

## Overview

Source and documentation preparation are independent deterministic pipelines. Optional documentation vectors are built separately and never become a prerequisite for exact or BM25 source retrieval.

Grimoire separates source preparation from documentation indexing and optional documentation-vector construction. Source retrieval remains usable without an embedding service.

## Prepared source index

```bash
grimoire index --root <repository>
```

The indexer resolves the repository and state roots, applies traversal rules, optionally resolves the current immutable Lexicon export, normalizes eligible text into semantic declaration chunks plus fallback gaps, computes immutable identities and exact token counts, builds a persistent identifier-aware lexical sidecar, reuses unchanged objects, and atomically publishes a prepared snapshot. That snapshot remains usable for BM25 and exact retrieval without an embedding service or query-time corpus tokenization.

`grimoire status --refresh` is the bounded preparation entry point for callers that need all deterministic repository state ready before a query. It checks the Git/source identity, refreshes Lexicon when absent or stale, synchronizes Arcana to the exact Lexicon snapshot, and then prepares Grimoire's source index. It never builds vectors; vector availability is reported separately.

## Permanent exclusions

These directory names are excluded at every traversal depth:

```text
.git
.grimoire
.ddocs
.lexicon
.arcana
.warlock
.worktrees
.workingtrees
```

The resolved `--state` path is also excluded, including a custom path with another name. These exclusions protect repository metadata, generated tool state, and nested worktree containers and cannot be re-included with ignore negation.

## Git-ignore behavior

Without `--ignore-file`, Grimoire loads the root `.gitignore` and nested `.gitignore` files as their directories are entered. Patterns use go-git's Git-ignore implementation and preserve normal scope and `!` negation behavior.

`--ignore-file` replaces the root and nested hierarchy with one explicit Git-ignore-syntax file. The control file itself is excluded. A missing explicit ignore file is an error.

By default, Grimoire also omits common dependency and generated-output directories (`node_modules`, `vendor`, `third_party`, `third-party`, `.next`, `.nuxt`, `coverage`, `dist`, `target`, and `out`). It skips common lockfiles, generated-code filename suffixes and headers, minified or bundled web assets, and large web/data files that are effectively one minified line. Small authored assets remain eligible. Pass `--include-generated` to bypass this policy; Git-ignore and explicit exclusions still apply.

## Supported files

Source and script extensions:

```text
.go .rs .py .rb .js .jsx .ts .tsx .java
.c .h .cc .cpp .hpp .cs .gd .sh .ps1
```

Documentation, configuration, and data extensions:

```text
.md .txt .toml .yaml .yml .json .xml
.html .css .scss .sql
```

Recognized extensionless names, matched case-insensitively:

```text
README LICENSE Makefile Dockerfile Gemfile Rakefile
```

An eligible entry must be a regular supported file, no larger than the configured maximum, and text-like. The current text check rejects files containing a NUL byte. Symlinks and other non-regular entries are not indexed. The default maximum is 2 MiB; a positive `--max-file-bytes` replaces it.

## Incremental identity and reuse

Grimoire computes SHA-256 over each eligible file and a separate preparation hash over the chunking contract plus that file's normalized Lexicon source spans. A prior file record is reused only when content hash, byte size, and preparation hash match. Reused records retain their existing chunks, semantic metadata, IDs, and token counts. New or changed files, or files whose Lexicon boundaries changed, are fully re-chunked.

A prior record is removed when its path is deleted, ignored, unsupported, oversized, binary, or otherwise absent from the eligible traversal result. Renames naturally reuse immutable content where the storage identity permits it while publishing the new path record.

Changing traversal, chunking, tokenizer, or schema behavior invalidates the relevant identity and forces affected work to be rebuilt. Semantic chunk metadata, preparation hashes, and persistent lexical postings use prepared-index format version 4, so older prepared state is rebuilt once on the next `grimoire index` run.

## Lexicon-aligned semantic chunking

When `<root>/.lexicon/CURRENT` exists, `grimoire index` resolves or creates the same immutable JSONL export used by structural retrieval. Declaration-level Lexicon spans become source boundaries for functions, methods, tests, and types. Nested closures and other callable-owned declarations do not fragment their owning declaration; smaller data-flow symbols remain inside the surrounding source chunk.

Nested declarations are represented by their narrowest non-overlapping leaves. For example, methods are prepared independently instead of duplicating the complete containing type. Source outside those semantic ranges remains indexed through the fallback chunker, so comments, package declarations, configuration, unsupported constructs, and files without Lexicon facts are not discarded.

Every semantic chunk stores the Lexicon kind and symbol name. A declaration above 1,536 tokens is split only for the hard token ceiling while retaining that semantic identity on each part.

`--lexicon-facts` selects an explicit JSONL export. `--lexicon-state` and `--lexicon-command` override automatic immutable-state discovery. An unreadable explicit facts export is an error; unavailable automatically discovered state warns and preserves fallback-only indexing.

## Fallback chunking

The language-agnostic fallback chunker:

- normalizes CRLF to LF;
- removes one final newline;
- skips empty or whitespace-only files;
- targets roughly 48 lines per chunk;
- prefers a recent blank-line boundary after at least eight useful lines;
- trims blank lines at chunk edges;
- enforces an exact 1,536-token ceiling after line-based chunking;
- recursively splits oversized chunks at line boundaries;
- falls back to token slices only when one source line alone exceeds the ceiling; and
- derives chunk identity from path, source range, exact text, and token-slice position when required.

Fallback boundaries are used for files with no valid Lexicon spans and for every uncovered region around semantic declarations.

## Token accounting

Changed chunks are counted with the embedded `o200k_base` tokenizer and store the exact count in prepared state. The manifest records tokenizer identity so counts cannot be reused under a different tokenizer.

Chunk counts cover chunk text only. Discovery responses apply independent lane limits and bounded excerpts; Grimoire does not token-fit a preassembled context package.

## Index statistics

The command reports:

- `scanned`: eligible files evaluated after filtering;
- `reused`: scanned files using prior records;
- `updated`: new or changed scanned files rebuilt; and
- `removed`: prior records absent from the new snapshot;
- `generated_skipped`: generated files or generated-directory roots omitted by the default policy;
- `semantic_files`: prepared files containing at least one Lexicon-aligned chunk; and
- `semantic_chunks`: chunks carrying a Lexicon declaration kind and name.

For a successful run:

```text
scanned = reused + updated
```

## Documentation indexing and vectors

Source preparation does not embed source chunks. To build the independent knowledge lane:

```bash
grimoire knowledge index --root <repository>
grimoire vector build --root <repository>
```

The knowledge index discovers documentation and rationale, extracts stable sections, and publishes BM25 terms plus exact citation handles under `.grimoire/knowledge/`. The optional vector builder deduplicates identical section text, reuses immutable content-addressed objects, embeds only missing sections, persists successful batches immediately, and publishes a packed snapshot bound to the exact knowledge-index identity.

A documentation change can make the vector snapshot stale without affecting the prepared source index. When vectors are explicitly requested, document discovery validates freshness before query embedding and retains BM25 results when vectors are missing, stale, or unavailable. Exact, source, symbol, relationship, trace, and impact operations never consume documentation vectors.

Run `grimoire index` after relevant source or source-indexing changes. Run `grimoire knowledge index` after documentation changes and `grimoire vector build` when semantic documentation ranking is desired. Use `grimoire vector info` to inspect snapshot freshness.

The `.grimoire/` directory is generated state and must not be treated as authored repository content.

## Code map

| Concern | Primary implementation | Related tests |
| --- | --- | --- |
| Repository walk and prepared source build | `internal/index/build.go`, `repository.go` | `internal/index/build_test.go` |
| Chunking and semantic boundaries | `internal/index/chunk.go`, `semantic.go` | `chunk_test.go`, `semantic_test.go` |
| Immutable object formats and publication | `internal/index/objects.go`, `codec.go`, `file_codec.go`, `store.go` | codec, object, and store tests |
| Generated-file and exclusion policy | `internal/index/generated.go`, `exclusions.go`, `internal/ignore/` | generated and app exclusion tests |
| Lexical sidecar | `internal/index/lexical.go`, `internal/lexical/` | lexical package tests |
| Repository freshness coordination | `internal/repostate/`, `internal/app/discovery_prepare.go` | repostate and discovery tests |
| Source retrieval consumers | `internal/retrieve/` | retrieve package tests and benchmarks |

Lexicon owns language facts used to improve chunk boundaries; the prepared index does not become a second language-analysis engine.

## Related docs

- [Prepared index architecture](../architecture/prepared-index.md)
- [Knowledge retrieval](knowledge.md)
- [Embedding model](embedding-model.md)
- [Current limitations](../limits/current-limitations.md)

## Notes

Generated `.grimoire/` state is never authored repository content and must remain excluded from indexing.
