# Arcana documentation

This directory is the authoritative documentation set for Arcana's current application behavior, implementation architecture, source ownership, development workflow, status, and focused contracts.

## Start here

| Document | Audience | Scope |
| --- | --- | --- |
| [Application and operations](APPLICATION.md) | Users and operators | CLI commands, state layout, synchronization, publication, consumer registration, locking, failures, and diagnostics |
| [Architecture](ARCHITECTURE.md) | Developers and integrators | Ownership boundaries, dependency direction, ingestion, compilation, storage, snapshots, protocol, vectors, and invariants |
| [Maintainer map](MAINTAINER_MAP.md) | Maintainers and agents | Short routing from common changes to canonical documents and implementation boundaries |
| [Development and verification](DEVELOPMENT.md) | Contributors | Prerequisites, builds, focused tests, complete verification, benchmarks, and documentation requirements |
| [Current status and limitations](STATUS.md) | Users and maintainers | Implemented capabilities, explicit limits, non-claims, and optional boundaries |

## Focused contracts

- [Lexicon contract](LEXICON_CONTRACT.md) — immutable snapshot ingestion, compatibility behavior, and incremental ownership.
- [Repository snapshots](repository-snapshots.md) — standalone snapshot artifacts, manifests, overlays, and changed-file updates.
- [Vector index](vector-index.md) — optional semantic graph documents, cache, index identity, build, resume, and query behavior.

## Related documentation

- [Arcana README](../README.md) — product overview, quick examples, graph workload rationale, and licensing.
- [Lexicon–Arcana analysis stack](../../docs/architecture/analysis-stack.md) — how Lexicon publication and Arcana synchronization fit together after Grimoire retirement.
- [Shared Arcana reference](../../docs/reference/arcana.md) — transitional shared reference retained during repository cleanup; this component documentation is authoritative.
- [Lexicon documentation](../../lexicon/docs/README.md) — the upstream language-analysis and snapshot producer.

## Status vocabulary

- **Implemented** means the behavior exists in the current Arcana source and is reachable through its CLI, protocol, or library boundary.
- **Optional** means the implementation exists but is not required for deterministic synchronization or graph traversal.
- **Compatibility behavior** means a bounded migration or degradation path exists and is documented explicitly.
- **Future possibility** means the idea is not current behavior or a compatibility guarantee.

Current-state documents must not describe a future possibility as implemented. Format and protocol claims must remain synchronized with source constants and tests.

## Placement rules

Place application behavior and operator procedures in `APPLICATION.md`. Place ownership and invariants in `ARCHITECTURE.md`. Place cross-document ownership routing in `MAINTAINER_MAP.md`; keep focused code maps in the owning documents. Place build and verification procedure in `DEVELOPMENT.md`. Place capability summaries and unresolved constraints in `STATUS.md`.

Normative focused contracts remain in their owning documents. Benchmark results are evidence for their exact options and execution environment, not permanent product guarantees.
