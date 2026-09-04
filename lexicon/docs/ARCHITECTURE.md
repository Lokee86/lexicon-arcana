# Lexicon architecture

Parent index: [Lexicon Documentation](README.md)

## Purpose

This document defines Lexicon's implemented ownership, analysis lifecycle, immutable storage model, incremental boundary, concurrency, publication, recovery, and compatibility seams.

## Overview

Lexicon transforms repository source into versioned language-neutral facts through independently executable adapters, content-addressed objects, immutable snapshots, and deterministic post-publication consumer handoff.

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
cmd/lexicon
    -> internal/cli
        -> internal/scan
            -> internal/files and internal/state
            -> internal/scope
            -> internal/adapters
            -> internal/objectstore
            -> internal/interstack
            -> internal/consumer
        -> internal/watch

adapters/<language>
    -> facts-v1 JSONL

.lexicon/
    -> immutable objects and manifests
    -> CURRENT publication pointer
```

### CLI

`cmd/lexicon` delegates command behavior to `internal/cli`. The CLI resolves a repository, loads its configuration, and invokes one bounded operation. `lexicon demon` is the exception: it remains active only to convert filesystem events into the same scan transaction used by `lexicon scan`.

### Repository state and file discovery

`internal/files` defines relevant source discovery and permanent exclusions. `.lexiconignore` adds repository-specific gitignore-compatible exclusions but cannot re-include permanent state, dependency, or build directories.

`internal/state` maintains a private source mirror beneath `.lexicon/repo`. Its Git repository is a change detector between successful Lexicon publications. It is deliberately not a second user-facing source history.

### Adapter orchestration

`internal/adapters` locates and invokes the self-contained language adapters. Adapters receive a repository root, output path, optional changed and removed file scopes, and optional parallel-execution parameters. They emit one deterministic facts-v1 JSONL stream.

Adapters do not write Lexicon snapshots directly and do not contain consumer-specific graph or retrieval policy.

### Cross-stack resolution

`internal/interstack` runs after the selected language adapters have produced a complete candidate manifest. It consumes their immutable facts, maps framework and transport declarations back to existing language-owned nodes, and emits one synthetic `interstack` facts library.

The initial resolver covers HTTP requests and routes, packet/message producers and consumers, and shared environment/configuration keys. Its synthetic nodes are contracts, not replacements for language symbols. Cross-stack edges retain source evidence, confidence, and unresolved outcomes rather than converting ambiguous string matches into definite graph relationships.

Arcana stores and traverses these relationships. Lexicon owns discovering them because route DSLs, packet construction, handler registration, and configuration access require source- and framework-aware analysis.

### Analysis planning

`internal/scan` compares the current relevant source tree with the last successfully published source state. It selects either:

- complete-language analysis; or
- a scoped analysis containing impacted owners, required dependency context, and language configuration files.

The planner treats correctness as the priority. Structural changes, invalid prior state, unsupported ownership, unsafe topology changes, or scoped adapter failure trigger complete-language analysis.

### Object storage

`internal/objectstore` parses validated adapter output and partitions records into:

- one immutable object per owned source file; and
- an optional shared object for unowned synthetic language facts.

New objects use the deterministic binary format in `spec/objects-v1.md`. Object identity is content-addressed. Existing bytes under an object ID are immutable.

A snapshot manifest references every object required for one complete repository analysis state. `CURRENT` is replaced atomically only after all referenced objects and the manifest are durable.

### Consumers

`internal/consumer` manages deterministic one-shot consumers registered under `.lexicon/consumers/`. Consumers run after successful publication or confirmation of the current snapshot. A consumer failure does not invalidate the already-published Lexicon snapshot.

Consumers receive the repository, state root, and snapshot ID through environment variables and should read only the immutable manifest and objects referenced by that snapshot.

## Analysis lifecycle

A normal scan performs this sequence:

1. resolve the initialized repository and acquire the repository update lock;
2. load configuration, the current manifest, and any recoverable pending publication;
3. mirror the current relevant source tree;
4. calculate changed, added, deleted, renamed, and configuration paths;
5. determine affected languages and safe complete or scoped plans;
6. execute adapters under the resource scheduler;
7. validate and parse each facts-v1 stream once;
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

A changed source file does not automatically imply a full language scan. Lexicon starts from the previous immutable snapshot and follows cross-file relationships in reverse to identify transitive dependents. Owners with unresolved relationships are included conservatively.

The scoped repository includes:

- impacted owners;
- their required forward dependency context;
- language configuration files;
- complete packages for Go;
- complete crates for Rust.

A scoped stream may replace only facts owned by its declared changed files. Partial shared facts cannot replace the previous complete shared object.

Lexicon retries with complete-language analysis when:

- a direct edit previously owned cross-file or unresolved relationships;
- the scoped result introduces relationship or unresolved topology that cannot be proven safe;
- an adapter emits the wrong stream mode;
- scoped execution fails;
- additions, deletions, renames, copies, configuration changes, or invalid prior state make ownership uncertain.

This fallback is part of the correctness design, not an error condition.

## Concurrency model

Full scans may run independent language plans concurrently. A weighted scheduler limits their combined reserved CPU weight to the process-wide `GOMAXPROCS` budget.

The Go adapter has a second level of parallelism:

1. Lexicon inventories Go source count and bytes.
2. It selects a repository-size-dependent logical shard count.
3. It chooses a bounded active worker count.
4. Typed call and dataflow work executes in shard-local scanners.
5. Results merge through a deterministic fan-in reduction tree.
6. Repository-wide SSA/VTA resolution runs after the typed shard merge.

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

Internal package structure, temporary adapter JSONL paths, private mirror implementation, scheduling heuristics, and binary storage implementation details outside the versioned object contract may change without becoming public application APIs.

## Code map

| Architecture boundary | Primary implementation | Related tests |
| --- | --- | --- |
| Scan lifecycle and planning | `internal/scan/` | `internal/scan/*_test.go` |
| Adapter registry, fingerprinting, and execution | `internal/adapters/`, `internal/languages/` | package-local tests |
| Scoped repositories and dependency expansion | `internal/scope/`, `internal/objectstore/dependencies.go`, `internal/scan/plan.go` | scope, dependency, and plan tests |
| Immutable objects, manifests, and recovery | `internal/objectstore/` | `internal/objectstore/*_test.go` |
| Private mirror and Git-backed change detection | `internal/state/`, `internal/files/` | state and files tests |
| Interstack contracts | `internal/interstack/`, `internal/scan/interstack.go` | interstack and scan integration tests |
| Consumer publication boundary | `internal/consumer/` | consumer tests |
| Concurrency and writer safety | `internal/scan/resource_scheduler.go`, `parallel_plan.go`, `internal/lock/` | scheduler, parallel-plan, and lock tests |

Adapters own parsing and semantic resolution. Arcana owns graph compilation and queries. Lexicon architecture ends at normalized immutable facts and consumer publication.

## Tests

Architecture invariants are protected by package-local tests under `internal/scan/`, `internal/objectstore/`, `internal/state/`, `internal/files/`, `internal/scope/`, `internal/watch/`, `internal/lock/`, `internal/consumer/`, and `internal/interstack/`, plus the complete adapter test matrix.

## Related docs

- [Application and operations](APPLICATION.md)
- [Dependency semantics](DEPENDENCY_SEMANTICS.md)
- [Development and verification](DEVELOPMENT.md)
- [Maintainer map](MAINTAINER_MAP.md)

## Notes

Arcana graph compilation and higher-level repository-discovery/agent workflow remain outside Lexicon's ownership boundary.
