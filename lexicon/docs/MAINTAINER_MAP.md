# Lexicon Maintainer Map

Parent index: [Lexicon Documentation](README.md)

## Purpose

This document routes common Lexicon changes to their canonical documentation and implementation boundary. It intentionally avoids enumerating every source file.

## Overview

Use this page to select the owning Lexicon document or adapter README. Continue in that document's focused code map for exact implementation and test paths.

## Change routing

| Change area | Canonical documentation | Primary implementation boundary |
| --- | --- | --- |
| Commands and operator behavior | [Application](APPLICATION.md) | `../lexicon-cli/src/` |
| Scan planning, publication, recovery, and concurrency | [Architecture](ARCHITECTURE.md) | `src/scan/`, `src/repository/`, `src/watch/` |
| Immutable facts, objects, snapshots, export, and GC | [Architecture](ARCHITECTURE.md), specifications under `spec/` | `src/storage/` |
| Adapter contract, discovery, and native execution | [Application](APPLICATION.md), [Adapters](../adapters/README.md), [Rust migration](RUST_MIGRATION.md) | `src/adapters/`, `src/languages/` |
| Language semantics | Owning adapter README and [Rust migration](RUST_MIGRATION.md) | native `src/adapters/<language>/`; transitional/oracle `adapters/<language>/` |
| Dependency and incremental scope semantics | [Dependency semantics](DEPENDENCY_SEMANTICS.md) | adapter dependency emitters, `src/scan/`, `src/repository/`, `src/scope.rs` |
| Interstack contracts | [Architecture](ARCHITECTURE.md) | `src/interstack/` |
| Post-publication consumers | [Application](APPLICATION.md) | `src/consumer/`, `../lexicon-cli/src/commands_consumer.rs` |
| Build, tests, corpora, and semantic validation | [Development](DEVELOPMENT.md) | `tests/`, `evaluation/`, adapter parity/oracle tests |
| Release bundles and installer verification | [Release packaging](RELEASE_PACKAGING.md) | `../scripts/workflow.py`, `../scripts/test_workflow.py` |

## Boundaries

- Adapters emit normalized facts; they do not publish snapshots.
- The scanner publishes immutable state; it does not implement language semantics.
- Lexicon does not own graph traversal, ranking, or packed graph storage.
- Focused implementation paths and tests belong in the `## Code map` section of the owning document or adapter README.

## Related docs

- [Architecture](ARCHITECTURE.md)
- [Application](APPLICATION.md)
- [Development](DEVELOPMENT.md)
- [Status](STATUS.md)

## Notes

Start here when ownership is unclear, then continue in the subject-specific document.
