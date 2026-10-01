# Lexicon architecture

Parent index: [Lexicon Documentation](README.md)

## Purpose

This document defines Lexicon's implemented ownership, analysis lifecycle, immutable storage model, incremental boundary, concurrency, publication, recovery, and compatibility seams.

## Overview

Lexicon transforms repository source into versioned language-neutral facts through language adapters, content-addressed objects, immutable snapshots, and deterministic post-publication consumer handoff.

Lexicon is an on-demand repository analysis application with an optional watch mode. It owns language extraction, normalized facts, incremental analysis decisions, immutable fact storage, and atomic snapshot publication.

It is not a general graph database, retrieval engine, documentation manager, or version-control system.

## Ownership boundaries

### Lexicon owns

- one reusable semantic adapter per supported language surface;
- stable cross-tool node identities and normalized relationship vocabulary;
- source spans, provenance, file ownership, and unresolved evidence;
- complete and incremental adapter execution;
- content-addressed per-file and shared-language fact objects;
- immutable snapshot manifests and the `CURRENT` publication pointer;
- post-publication consumer registration and invocation;
- deterministic export back to facts-v1 JSONL;
- repository-wide cross-stack contract resolution over adapter facts and source evidence.

### Lexicon does not own

- packed graph query execution, reachability, impact analysis, or graph traversal;
- lexical or vector retrieval and context-package construction;
- documentation policy or automated documentation repair;
- runtime instrumentation providers;
- source-repository history or branch management.

Arcana may consume Lexicon snapshots, but Lexicon remains independently executable and does not require Arcana.

## Component map

```text
lexicon-cli/src
    -> lexicon public API
        -> src/repository + src/config
        -> src/scan + src/scope
        -> src/adapters
        -> src/storage
        -> src/interstack
        -> src/consumer
        -> src/watch

language adapters
    -> versioned typed LanguageAdapter contract

.lexicon/
    -> immutable objects and manifests
    -> CURRENT publication pointer
```

### CLI

The separate Rust `lexicon-cli` crate is a thin host over the Rust `lexicon` library. It resolves command-line inputs and invokes one bounded library operation. `lexicon demon` is the exception: it remains active only to convert filesystem events into the same scan transaction used by `lexicon scan`.

### Repository state and file discovery

The Rust repository/configuration modules define relevant source discovery and permanent exclusions. `.lexiconignore` adds repository-specific gitignore-compatible exclusions but cannot re-include permanent state, dependency, or build directories.

The Rust repository state layer maintains a private source mirror beneath `.lexicon/repo`. Its Git repository is a change detector between successful Lexicon publications. It is deliberately not a second user-facing source history.

### Adapter orchestration

The Rust `src/adapters` boundary owns a versioned `LanguageAdapter` contract. Adapters receive a typed request containing the repository root, optional changed and removed file scopes, and optional parallel-execution parameters, then return typed `Analysis` facts directly. The host validates the adapter-contract and fact-schema versions, canonicalizes record ordering, then validates fact invariants and emitted language before materialization. There is no adapter output path, subprocess fallback, or JSONL handoff in the Rust scan path.

Adapters do not write Lexicon snapshots directly and do not contain consumer-specific graph or retrieval policy. The older Go process adapter layer remains only as migration-oracle source while those language implementations are translated.

### Cross-stack resolution

`src/interstack/` runs after the selected language adapters have produced a complete candidate manifest. It consumes their immutable facts, maps framework and transport declarations back to existing language-owned nodes, and emits one synthetic `interstack` facts library.

The initial resolver covers HTTP requests and routes, packet/message producers and consumers, and shared environment/configuration keys. Its synthetic nodes are contracts, not replacements for language symbols. Cross-stack edges retain source evidence, confidence, and unresolved outcomes rather than converting ambiguous string matches into definite graph relationships.

Arcana stores and traverses these relationships. Lexicon owns discovering them because route DSLs, packet construction, handler registration, and configuration access require source- and framework-aware analysis.

### Analysis planning

`src/scan/` compares the current relevant source tree with the last successfully published source state. It selects either:

- complete-language analysis; or
- a scoped analysis containing impacted owners, required dependency context, and language configuration files.

The planner treats correctness as the priority. Structural changes, invalid prior state, unsupported ownership, unsafe topology changes, or scoped adapter failure trigger complete-language analysis.

### Object storage

The storage layer consumes validated typed adapter analysis and partitions records into:

- one immutable object per owned source file; and
- an optional shared object for unowned synthetic language facts.

New objects use the deterministic binary v2 format in `spec/objects-v2.md`; binary v1 and legacy JSON remain readable. Object identity is content-addressed. Existing bytes under an object ID are immutable.

A snapshot manifest references every object required for one complete repository analysis state. Full analysis builds its optional partitioned dependency index directly from grouped facts. Each safe incremental publication reuses unaffected immutable shards and writes only changed ownership, reference, unresolved-candidate and adjacency partitions, then embeds a new index root bound to the exact resulting language entry. Version 2 roots also provide shared-node path lookup; legacy version 1 roots remain readable. A legacy unindexed snapshot bootstraps its own derived index once. Before `CURRENT` advances, Lexicon verifies the index root and every referenced immutable shard, including during recovery. GC preserves index roots and shards reachable from current, retained and consumer-pinned snapshots and removes stale bootstrap directories. The index is not needed by consumers to read facts.

### Consumers

`src/consumer/` manages deterministic one-shot consumers registered under `.lexicon/consumers/`. Consumers run after successful publication or confirmation of the current snapshot. A consumer failure does not invalidate the already-published Lexicon snapshot.

Consumers receive the repository, state root, and snapshot ID through environment variables and should read only the immutable manifest and objects referenced by that snapshot.

## Analysis lifecycle

A normal scan performs this sequence:

1. resolve the initialized repository and acquire the repository update lock;
2. load configuration, the current manifest, and any recoverable pending publication;
3. mirror the current relevant source tree;
4. calculate changed, added, deleted, renamed, and configuration paths;
5. determine affected languages and safe complete or scoped plans;
6. execute adapters under the resource scheduler;
7. validate each adapter's typed analysis against the current adapter and facts contracts;
8. build replacement owned objects and reuse unaffected manifest entries;
9. derive the synthetic repository-wide `interstack` library from the candidate language facts;
10. write missing immutable objects;
11. write the durable `PENDING` candidate;
12. advance the private source-state commit when required;
13. write the immutable snapshot manifest;
14. atomically replace `CURRENT` and remove `PENDING`;
15. invoke registered consumers;
16. release the lock.

A scan that produces the same complete manifest confirms the existing snapshot instead of publishing duplicate mutable state.

## Incremental correctness boundary

A changed source file does not automatically imply a full language scan. Lexicon starts from the previous immutable snapshot and includes direct one-hop reverse dependents; forward context adds one hop from the emitted set. Sensitive unresolved Python module candidates and unproven topology changes trigger conservative full-language analysis. Newly indexed language generations read only visited immutable topology partitions, not every unrelated fact object. Legacy generations bootstrap that index once under a pointer bound to the exact old snapshot.

The scoped repository includes:

- impacted owners;
- their required forward dependency context;
- language configuration files;
- complete packages for Go;
- complete crates for Rust.

A scoped analysis may replace only facts owned by its declared changed files. Partial shared facts cannot replace the previous complete shared object.

Lexicon retries with complete-language analysis when:

- a direct edit previously owned cross-file or unresolved relationships;
- the scoped result introduces relationship or unresolved topology that cannot be proven safe;
- an adapter emits the wrong analysis mode;
- scoped execution fails;
- additions, deletions, renames, copies, configuration changes, or invalid prior state make ownership uncertain.

This fallback is part of the correctness design, not an error condition.

## Concurrency model

Full scans may run independent language plans concurrently. A weighted scheduler limits their combined reserved CPU weight to the process-wide `GOMAXPROCS` budget.

Adapters that advertise the partitioned-execution capability have a second level of parallelism:

1. Lexicon inventories adapter-owned source count and bytes.
2. It selects a repository-size-dependent logical shard count.
3. It chooses a bounded active worker count.
4. Adapter-local semantic work executes in shard-local scanners.
5. Results merge through a deterministic fan-in reduction tree.
6. Repository-wide resolution runs after the local shard merge.

Logical shards are work partitions, not simultaneous workers. The planner may create many logical shards while activating only a bounded number of workers. `LEXICON_MAX_WORKERS` can lower the active-worker ceiling.

All supported worker counts and merge shapes must produce byte-identical facts.

## Multi-module Go ownership

The Go adapter discovers every `go.mod` beneath the scanned repository, assigns each Go source file to its nearest module root, and analyzes modules independently before deterministic fact merge. Repository identity is the root module path when a root module exists; otherwise it is the repository directory name.

Package and symbol identities remain module-qualified. Nested modules do not inherit the parent module path.

## Publication and recovery

`PENDING` records the complete candidate manifest before mutable source-state advancement. Recovery distinguishes three cases:

- source state did not advance: discard the candidate and recompute;
- source state advanced but publication did not: attach the committed state and publish without rerunning adapters;
- publication requires no source-state change: publish the durable candidate directly.

Consumers resolve `CURRENT` once and then read immutable data. They observe either the previous complete snapshot or the new complete snapshot, never a partially written analysis state.

## Compatibility boundaries

The public compatibility surfaces are the versioned contracts under `spec/`, the CLI behavior documented in `docs/APPLICATION.md`, and the consumer definition format.

Internal package structure, private mirror implementation, scheduling heuristics, and binary storage implementation details outside the versioned object contract may change without becoming public application APIs. The adapter boundary is explicitly versioned so its typed contract can evolve without treating transport details as public API.

## Code map

| Architecture boundary | Primary implementation | Related tests |
| --- | --- | --- |
| CLI host | `../lexicon-cli/src/` | `../lexicon-cli/tests/`, module tests |
| Scan lifecycle and planning | `src/scan/`, public entry points in `src/api/` | scan-engine, scan-execution, planning, transaction, and public-API tests |
| Adapter contract, registry, fingerprinting, and native execution | `src/adapters/`, `src/languages/` | adapter-host/registry and language tests |
| Scoped repositories and dependency expansion | `src/scan/`, `src/repository/`, `src/scope.rs` | scope-execution, dependency-topology, and planning tests |
| Immutable objects, manifests, and recovery | `src/storage/`, publication/recovery support in `src/repository/` and `src/scan/` | storage, publication, and recovery tests |
| Private mirror and Git-backed change detection | `src/repository/` | private-state/source-mirror tests |
| Interstack contracts | `src/interstack/` | interstack boundary/build/resolve tests |
| Consumer publication boundary | `src/consumer/` | consumer-execution tests |
| Watch and cancellation | `src/watch/` | watch-daemon tests |

Adapters own parsing and semantic resolution. Arcana owns graph compilation and queries. Lexicon architecture ends at normalized immutable facts and consumer publication.

## Tests

Architecture invariants are protected by Rust integration/module tests across `tests/`, `src/scan/`, `src/storage/`, `src/repository/`, `src/watch/`, `src/consumer/`, `src/interstack/`, and `src/adapters/`, plus the remaining parity-oracle suites documented in `RUST_MIGRATION.md`.

## Related docs

- [Application and operations](APPLICATION.md)
- [Dependency semantics](DEPENDENCY_SEMANTICS.md)
- [Development and verification](DEVELOPMENT.md)
- [Maintainer map](MAINTAINER_MAP.md)

## Notes

Arcana graph compilation and higher-level repository-discovery/agent workflow remain outside Lexicon's ownership boundary.
