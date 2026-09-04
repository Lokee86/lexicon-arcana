# Knowledge retrieval

Parent index: [Reference](INDEX.md)

Historical status: this page describes the retired Grimoire document-retrieval layer and is retained only for prior benchmark/design interpretation. It is not an active product contract.

## Purpose

This document defines Grimoire's independent repository-documentation discovery, indexing, BM25 search, stable handles, optional vectors, and judged evaluation surface.

## Overview

Documentation evidence remains separate from source and structural evidence. Explicit code links and snapshot freshness improve navigation without allowing prose to override contradictory implementation evidence.

`grimoire knowledge` indexes repository rationale separately from production-source chunks. It discovers Markdown/MDX and text documentation, architecture and planning notes, ADR-like files, issue/export notes, and supported prose-heavy configuration. `.git`, worktrees, Lexicon/Arcana/Grimoire state, generated content, `.gitignore` matches, and explicit exclusions are not indexed.

```bash
grimoire knowledge index --root .
grimoire knowledge search --root . --query "why is snapshot freshness validated" --top-k 5
grimoire knowledge inspect --root . --handle 'knowledge://docs/architecture/index.md#...'
grimoire knowledge vector build --root .
grimoire knowledge vector info --root .
```

The root aliases `grimoire vector build` and `grimoire vector info` invoke the same documentation-vector commands. There is no production source-vector build or search command.

## Persistent state

The knowledge index defaults to `.grimoire/knowledge/index.json`. Documents and sections have stable path/heading-derived identities, exact byte and line spans, hashes, Git metadata when available, and deterministic BM25 terms.

Optional vectors live under `.grimoire/knowledge/vectors/<embedding-identity>/`. Their manifest is bound only to documentation paths, kinds, document hashes, section IDs, and section hashes. Repository moves, new commits, and unrelated source-code changes do not invalidate a current documentation snapshot. Documentation content or section-structure changes do.

## Search

Knowledge search always runs deterministic BM25 first. It supports `--path`, `--kind`, `--heading`, `--commit`, `--since`, and `--until` filters and returns cited section text, stable handles, ranking reasons, and exact code-link hints when documented symbols, repository paths, endpoints, messages, or configuration contract names also occur in repository files.

Search is BM25-only by default. Pass `--vectors=true` to embed the query and add a bounded supplemental vector score through the `knowledge.VectorRanker` seam when a current snapshot exists. BM25 remains the primary ranking path.

Missing, stale, incompatible, timed-out, or unavailable vectors do not fail knowledge retrieval. The response keeps BM25 results, sets `vector_used` to false, and exposes the failure through `vector_error`. Freshness is validated before query embedding.

## Build behavior

The vector builder deduplicates identical section text, reuses immutable content-addressed vector objects, writes successful batches immediately, and publishes a packed exact-search snapshot only after all required vectors are available. A failed build leaves completed objects reusable by the next run. An unchanged current snapshot returns immediately without object probes or rematerialization.

Documentation vectors are consumed only when explicitly requested by `knowledge search --vectors=true`, discovery `--document-vectors`, or MCP `use_document_vectors: true`. They never enter exact, source, symbol, or relationship ranking.

## Judged documentation evaluation

`grimoire eval knowledge` runs a frozen documentation corpus against this same BM25 and optional-vector seam. It does not build an index or alter source retrieval:

```bash
grimoire eval knowledge --root . --cases evaluation/knowledge/grimoire.json --vectors=false
```

The report keeps corpus order and records required-section recall, recall@k, MRR, irrelevant selections, vector usage/error, latency, and every returned section for each case. Enable vectors for a supplemental comparison; missing or failed vectors are reported without replacing BM25 results.

## Code map

| Concern | Primary implementation | Related tests |
| --- | --- | --- |
| Document discovery and section extraction | `internal/knowledge/discover.go`, `sections.go`, `types.go` | `internal/knowledge/knowledge_test.go` |
| Persistent document state and identity | `internal/knowledge/store.go`, `identity.go` | identity and knowledge tests |
| Lexical document search | `internal/knowledge/search.go` | knowledge tests and evaluation corpus |
| Explicit documentation-to-code links | `internal/knowledge/links.go` | `internal/knowledge/links_test.go` |
| Optional document vectors | `internal/knowledgevector/` | package-local `*_test.go` files |
| CLI preparation and vector commands | `internal/app/knowledge.go`, `knowledge_vectors.go` | app vector tests |
| Judged evaluation | `internal/knowledgeevaluation/`, `evaluation/knowledge/` | `internal/knowledgeevaluation/score_test.go` |

Documentation evidence remains a distinct lane and does not override contradictory source or graph evidence.

## Related docs

- [Indexing](indexing.md)
- [Embedding model](embedding-model.md)
- [Vector store](vector-store.md)
- [Discovery quality](../development/retrieval-quality.md)

## Notes

Planning and research documents may be indexed as evidence, but they do not become canonical owners of current implementation behavior.
