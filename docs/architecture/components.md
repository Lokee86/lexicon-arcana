# Component architecture

Parent index: [Architecture](INDEX.md)

## Purpose

This document defines component ownership, dependency direction, state ownership, independent-use rules, and release boundaries for Grimoire, Lexicon, and Arcana.

## Overview

The three applications are packaged together but remain independently usable products with separate executables, private state, implementation domains, tests, and specialist command surfaces.

Grimoire, Lexicon, and Arcana share one repository but retain separate ownership, state, and advanced command surfaces.

## Grimoire

The repository-root Go application is the primary discovery interface. It owns:

- exact and BM25 source discovery;
- independent documentation discovery;
- stable handles and progressive inspect, trace, and impact operations;
- repository-state preparation and provider routing;
- investigation-session deduplication;
- the CLI and MCP discovery contracts.

Grimoire does not own language parsing or graph semantics. It exposes `grimoire lexicon ...` and `grimoire arcana ...` as thin product namespaces that delegate specialist operations to the owning binaries.

## Lexicon

`lexicon/` owns:

- one adapter per supported programming language;
- normalized symbols, spans, and relationships;
- immutable analysis objects and snapshots;
- incremental and Git-aware source analysis;
- standalone scan, export, and inspection commands.

Grimoire consumes Lexicon snapshots through immutable exports. Lexicon does not depend on Grimoire or Arcana.

## Arcana

`arcana/` owns:

- ingestion of one immutable Lexicon snapshot;
- packed forward and reverse repository graphs;
- neighbors, paths, impact, call chains, unresolved references, and graph inspection;
- optional semantic graph entry points;
- standalone graph commands and protocol behavior.

Arcana does not own language adapters or Grimoire's discovery response. Optional semantic indexing uses a compatible external embedding endpoint.

## Dependency direction

```text
repository source
    -> Lexicon snapshot
        -> Arcana graph snapshot

repository source and documentation
    -> Grimoire prepared state

Grimoire discovery
    -> reads Lexicon snapshot
    -> queries Arcana graph
    -> returns one provider-neutral response
```

Lexicon is upstream of Arcana. Grimoire may consume both but neither component calls back into Grimoire for deterministic analysis.

## Independent use

- Lexicon can analyze and export source facts without Arcana or Grimoire.
- Arcana can synchronize and answer graph queries without Grimoire.
- Grimoire can return exact, source, and document evidence without Lexicon or Arcana.
- Missing structural providers reduce available lanes and produce warnings; they do not invalidate unrelated evidence.

## State ownership

| State | Owner |
| --- | --- |
| `.grimoire/` source index | Grimoire |
| `.grimoire/knowledge/` documents and optional vectors | Grimoire |
| `.lexicon/` immutable language-analysis snapshots | Lexicon |
| `.arcana/` graph snapshots and optional graph vectors | Arcana |

Consumers interact through immutable manifests, exported facts, stable handles, and explicit protocols. No component mutates another component's state directly.

## Build and release boundaries

The repository-root workflow builds and packages the components together while preserving separate binaries:

- `grimoire`
- `lexicon`
- `arcana`

The workflow defaults to one build or test worker to avoid uncontrolled CPU fan-out. Higher concurrency requires an explicit `--jobs N`.

Each component may still be built and used independently from its owning source root. Ordinary product use does not require invoking those binaries directly: Grimoire resolves the bundled or configured provider and forwards namespaced commands while preserving process isolation and native exit behavior.

Lodestone remains a separately owned cross-repository native dependency. Grimoire pins the consumed Go module pseudo-version and exact source commit; local and release workflows verify that identity before tests or packaging. The checked-in local `replace` directive is a development override, not a second source authority.

## Product boundary

The active investigation path is Grimoire's progressive discovery interface. Direct Lexicon and Arcana commands remain available as namespaced specialist operations, not competing repository-discovery interfaces. The former context-package compiler is not part of the CLI or MCP contract. Historical package evaluators and reports do not define current architecture.

## Code map

| Component boundary | Primary implementation | Related tests |
| --- | --- | --- |
| Grimoire executable and command dispatch | `cmd/grimoire/main.go`, `internal/app/run.go` | `internal/app/run_test.go` |
| Grimoire provider forwarding | `internal/app/engine_commands.go`, `internal/app/engine_specs.go` | `internal/app/engine_commands_test.go` |
| Grimoire state alignment | `internal/repostate/`, `internal/app/discovery_prepare.go` | `internal/repostate/*_test.go`, `internal/app/discovery_test.go` |
| Lexicon executable and application boundary | `lexicon/cmd/lexicon/main.go`, `lexicon/internal/cli/` | `lexicon/internal/cli/*_test.go` |
| Lexicon publication boundary | `lexicon/internal/scan/`, `lexicon/internal/objectstore/` | package-local `*_test.go` files |
| Arcana executable and command boundary | `arcana/src/main.rs`, `arcana/src/cli.rs`, `arcana/src/cli_*.rs` | `arcana/src/cli*_tests.rs` |
| Arcana repository graph and protocol boundary | `arcana/src/repository/`, `arcana/src/protocol/`, `arcana/src/storage.rs`, `arcana/src/snapshot.rs` | Arcana module-local Rust tests |
| Shared graph kernel used by Arcana and other consumers | `arcana-graph/src/primitives.rs`, `arcana-graph/src/storage/`, `arcana-graph/src/snapshot/`, `arcana-graph/src/traversal.rs` | `cargo test --manifest-path arcana-graph/Cargo.toml --all-targets` |
| Release composition | `scripts/workflow.py`, `scripts/install.py`, `.github/workflows/release.yml` | `scripts/test_workflow.py`, installation smoke tests |

Grimoire must not absorb Lexicon's parsers or Arcana's storage internals. Cross-component changes should update the owning component document and the integration document together.

## Tests

Component boundaries are protected by root command-forwarding and repository-state tests, Lexicon application and publication tests, Arcana CLI and snapshot tests, and the combined build, installation, and release workflow tests.

## Related docs

- [Analysis stack](analysis-stack.md)
- [System overview](system-overview.md)
- [Grimoire maintainer map](maintainer-map.md)
- [Documentation coverage](../development/documentation-coverage.md)

## Notes

Cross-component convenience must not erase the component boundaries defined here.
