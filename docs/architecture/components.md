# Component architecture

Parent index: [Architecture](INDEX.md)

## Purpose

Define the active ownership, dependency direction, state, independent-use rules, and release boundaries for Lexicon and Arcana after Grimoire retirement.

## Overview

Lexicon and Arcana are complementary repository-analysis products with separate executables, implementation domains, state, tests, documentation, and direct command surfaces.

The active dependency direction is intentionally simple:

```text
repository source
    -> Lexicon semantic snapshot
        -> Arcana repository graph
            -> humans, agents, Warlock, Pitlord, other consumers
```

Ordinary source search, direct file reads, IDEs, and Git remain parallel development tools. They are not a hidden third repository-analysis component.

See [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md).

## Lexicon

`lexicon/` owns:

- one semantic adapter per supported programming-language surface;
- normalized symbols, spans, calls, relationships, dataflow, dependencies, and unresolved evidence;
- immutable content-addressed analysis objects and snapshots;
- incremental repository analysis, deterministic merge, and crash-safe publication;
- direct `init`, `scan`, `status`, `doctor`, `export`, and consumer commands.

Lexicon does not depend on Arcana. A published Lexicon snapshot remains valid if an optional downstream consumer fails.

## Arcana

`arcana/` owns:

- ingestion and verification of one immutable Lexicon snapshot;
- packed forward and reverse repository/call graphs;
- immutable snapshots, overlays, and compaction;
- symbol/file resolution, neighbours, impact, paths, call chains, unresolved references, graph statistics, operational roles, and architecture summaries;
- optional semantic graph entry points backed by a generic external embedding endpoint;
- direct CLI and `arcana.query.v1` protocol behavior.

Arcana does not own language parsing or adapter semantics. It preserves Lexicon's durable identities while using snapshot-local compact graph IDs internally.

## Grimoire retirement boundary

Grimoire is no longer an active component owner. Its source/document retrieval, stable handles, sessions, MCP, provider routing, and unified discovery response are retirement targets rather than responsibilities to be absorbed into Lexicon or Arcana.

Existing `.grimoire/` state and historical benchmark artifacts may remain during migration or for reproducibility, but they do not define current product behavior.

## State ownership

| State | Owner |
| --- | --- |
| `.lexicon/` immutable semantic-analysis snapshots | Lexicon |
| `.arcana/` graph snapshots and optional graph-vector state | Arcana |
| `.grimoire/` legacy discovery/index/session state | Retired; migration/history only |

No component mutates another component's private state directly. Integration uses immutable snapshots, manifests, exports, and explicit protocols.

## Independent use

- Lexicon can analyze and export repository facts without Arcana.
- Arcana can synchronize from Lexicon and answer graph queries without Grimoire.
- Consumers may read source or Git directly without routing through either product.
- Arcana synchronization may be registered as a deterministic Lexicon post-publication consumer, but Lexicon publication does not depend on Arcana success.

## Build and release boundary

The intended active release contains:

- `lexicon`;
- Lexicon runtime adapters;
- `arcana`;
- the production Lexicon + Arcana agent skill when that migration is complete.

The existing root Grimoire build/release machinery is transitional and will be removed or rewritten in the next retirement pass. A combined bundle may remain, but it is a Lexicon + Arcana bundle rather than a wrapper application.

## Consumer boundary

Higher-level products consume the owned component surfaces directly:

```text
Warlock / agent framework
    -> Lexicon snapshots/exports for semantic evidence
    -> Arcana protocol for bounded graph questions
    -> ordinary source/Git tools for exact implementation evidence
```

Warlock owns probabilistic task/context orchestration. Lexicon and Arcana remain deterministic analysis providers.

## Code map

| Boundary | Primary implementation | Related tests |
| --- | --- | --- |
| Lexicon executable and application | `lexicon/cmd/lexicon/main.go`, `lexicon/internal/cli/` | `lexicon/internal/cli/*_test.go` |
| Lexicon scan/publication | `lexicon/internal/scan/`, `lexicon/internal/objectstore/` | package-local `*_test.go` |
| Lexicon adapters | `lexicon/adapters/` | adapter-owned tests/evaluations |
| Arcana executable | `arcana/src/main.rs`, `arcana/src/cli.rs`, `arcana/src/cli_*.rs` | `arcana/src/cli*_tests.rs` |
| Arcana graph and snapshots | `arcana/src/repository/`, `arcana/src/storage/`, `arcana/src/snapshot/` | module-local Rust tests |
| Arcana query protocol | `arcana/src/protocol/` | protocol module tests |
| Transitional root release composition | `scripts/workflow.py`, `.github/workflows/release.yml` | `scripts/test_workflow.py` |

## Tests

Lexicon application/publication tests and Arcana ingestion/storage/snapshot/protocol tests are the active component-boundary verification. Root Grimoire-specific tests remain transitional until the implementation-removal pass.

## Related docs

- [Analysis stack](analysis-stack.md)
- [System overview](system-overview.md)
- [Architecture decisions](../decisions/INDEX.md)
- [Lexicon maintainer map](../../lexicon/docs/MAINTAINER_MAP.md)
- [Arcana maintainer map](../../arcana/docs/MAINTAINER_MAP.md)
