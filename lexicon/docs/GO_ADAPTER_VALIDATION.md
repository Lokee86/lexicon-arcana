# Go adapter real-repository validation

Parent index: [Lexicon Documentation](README.md)

## Purpose

This document records real-repository validation evidence for the Lexicon Go adapter and its Arcana-consumable output.

## Overview

The validation measures declaration, relationship, call-resolution, packed-graph, and query behavior on recorded repository revisions. It is evidence for those inputs, not a universal semantic guarantee.

## Research status

Retained dated validation evidence for the current Go adapter design. Results may be superseded by later runs but remain historical records.

## Question

Does the Go adapter produce deterministic, useful, and conservatively resolved facts on representative real repositories?

## Method

Run the adapter and downstream Arcana ingestion on pinned repositories, inspect semantic metrics and unresolved categories, and execute representative graph queries.

## Corpus or inputs

The exact repositories, revisions, commands, and measured outputs are recorded in the result sections below.

This is a dated validation record, not a current performance benchmark. It captures the Go semantic adapter validation performed on July 22, 2026 before the later multi-module repository and adaptive semantic-parallelism changes were merged into `main`. The semantic categories and limitations remain relevant; counts, file totals, packed sizes, and elapsed behavior may differ on the current implementation.

Validated on July 22, 2026 against two existing Go modules without modifying
either source repository.

## Results

| Repository | Nodes | Edges | Indexed files | Packages | Call expressions | Definite call expressions | Possible target facts | Conversion expressions | Unresolved calls | Closures | Captures | Packed graph | Catalogue | Unresolved file |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Demon Docs | 4,687 | 17,176 | 353 | 33 | 17,993 | 17,276 | 287 | 675 | 0 | 357 | 1,055 | 281,248 B | 539,274 B | 10 B |
| Space Rocks game server | 5,596 | 19,853 | 528 | 53 | 18,430 | 17,594 | 731 | 765 | 0 | 389 | 526 | 327,936 B | 762,943 B | 10 B |

`Definite call expressions` counts call sites with one callable contract. This
includes internal, standard-library, third-party, built-in, interface-contract,
and uniquely resolved function-value calls. `Possible target facts` counts
conservative runtime targets emitted for interface dispatch, callbacks, method
values, and other function flows; multiple targets may belong to one call site.
Type conversions are represented separately through `converts-to` edges.

Neither repository produced an unresolved call fact. This does not mean every
runtime dispatch is claimed to be uniquely known. Definite calls use `calls`;
multi-target dispatch uses `possible-calls`; conversions use `converts-to`.
Arcana therefore retains uncertainty instead of turning every possible target
into a definite edge.

Both repositories completed Go package/type loading with zero reported semantic
errors.

## Resolution behavior

The adapter combines repository-wide AST extraction with
`golang.org/x/tools/go/packages`, Go type information, SSA, and variable-type
analysis. It models:

- same-package and cross-package functions and methods;
- recursive calls as valid graph self-edges;
- standard-library and external API symbols without indexing dependency source;
- built-ins as callable symbol nodes;
- type conversions as `converts-to` relationships;
- interface types, interface methods, embedded interfaces, and implementation
  relationships;
- concrete interface targets as conservative `possible-calls` relationships;
- function variables, callback parameters, method values, and returned function
  values;
- closures as independent function nodes, including nested closure bodies;
- closure captures as variable nodes reached through `references` edges;
- mutually exclusive build-tag declarations under one canonical package-level
  symbol identity;
- AST-only callable contracts for files excluded from the active host build.

External closures and compiler-generated wrappers with no source declaration
receive stable synthetic nodes. Reflection-heavy or opaque calls can therefore
retain a callable contract even when no concrete repository implementation can
be proven.

## Packed graph results

Demon Docs produced these unique packed relationships:

- 10,909 `calls`;
- 279 `possible-calls`;
- 445 `converts-to`;
- 688 capture `references`;
- 45 `implements`;
- zero unresolved references.

Multiple call sites between the same two nodes and relation become one packed
edge, so packed relationship counts are intentionally lower than call-site
counts. The query protocol does not calculate a percentage from those
incompatible units.

## Query checks

A real Demon Docs closure at `internal/reverseindex/watch.go:26:9` returned:

- two conservative `possible-calls` targets, `Lock` and `Unlock`;
- seven captured variables through `references` edges;
- zero unresolved references in the snapshot.

The protocol statistics reported all new relation categories separately.
Both repositories imported successfully into verified repository snapshots.

An incremental test removed one real `possible-calls` edge from that closure.
`update-facts` reported one changed file, one removed edge, and an overlay rather
than a rebuild. The updated query returned one remaining target, and snapshot
diff reported exactly one relationship-changed source node.

Both fact files were generated twice and compared byte-for-byte. Repeated output
was identical.

## Remaining semantic limits

The remaining limits are about precision and provenance rather than missing
call records:

- VTA is conservative and can over-approximate runtime targets;
- build-tag variants currently share one logical symbol rather than carrying a
  per-build-configuration execution view;
- reflection, plugins, cgo, assembly, and runtime-generated functions may retain
  contracts or synthetic targets instead of one proven concrete implementation;
- generated-code classification is not yet represented explicitly;
- dependency implementation graphs require indexing those dependencies as
  separate repositories;
- packed graph edges are deduplicated relationships, so exact call-site coverage
  requires a future call-site fact layer rather than reconstruction from edges.

## Limitations

The corpus is finite, repository selection is not statistically representative, and static analysis cannot establish reflection, runtime registration, generated code, or unscanned external implementations.

## Interpretation

Passing results support the adapter's documented conservative semantics on the recorded revisions. They do not establish complete Go program behavior.

## Retained artifacts

Commands, repository revisions, measurement tables, unresolved classifications, and query checks are retained in this document and the referenced validation outputs.

## Related docs

- [Go adapter](GO_ADAPTER.md)
- [Semantic acceptance gates](SEMANTIC_ACCEPTANCE.md)
- [Semantic corpus validation](SEMANTIC_CORPUS_VALIDATION.md)
- [Development and verification](DEVELOPMENT.md)

## Notes

Repository revisions, adapter versions, commands, and accepted unresolved categories must remain attached to any quoted result.
