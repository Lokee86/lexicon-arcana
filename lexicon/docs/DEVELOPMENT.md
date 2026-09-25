# Lexicon development and verification

Parent index: [Lexicon Documentation](README.md)

## Purpose

Define the current source-build, focused-test, migration-parity, documentation, and release verification workflow for Lexicon.

## Overview

The Rust `lexicon` library plus the separate Rust `lexicon-cli` host are the active replacement implementation under migration. The recommended operator runtime remains the last optimized Go Lexicon revision, `758af9daf6e71fc0a7ebb837875efe366f6403fd`, because Rust optimization is not yet complete.

The root `scripts/workflow.py` command remains the canonical composition point for testing migration work and the current Lexicon + Arcana checkout.

## Prerequisites

The complete shared build currently requires:

- Python 3.12 or newer for the root workflow and validation tooling;
- Rust 1.90 or newer for the Lexicon library/CLI and Arcana;
- Go 1.26.5 for remaining parity/runtime adapters and their tests;
- Node.js 22 for the TypeScript adapter;
- JDK 21 or newer for Java analysis/release packaging;
- a compatible .NET SDK for the C# adapter;
- Ruby when exercising the Ruby oracle/runtime paths.

A focused change only needs the runtimes used by the affected component.

## Build the recommended Go runtime

For operator/performance validation, use the pinned Go reference revision `758af9daf6e71fc0a7ebb837875efe366f6403fd` and build from its `lexicon/` directory:

```text
go build -o ../bin/lexicon ./cmd/lexicon
```

Verify it with:

```text
../bin/lexicon version
```

## Build the Rust migration target

For migration development from the current shared repository root:

```text
python scripts/workflow.py build --version 0.1.0-dev --component lexicon
```

This produces the Rust migration executable under `build/bin/lexicon`. Use it for parity, optimization, and migration verification rather than as the default operator build until the optimization gap is closed.

## Focused Rust verification

Lexicon library:

```text
cargo fmt --manifest-path lexicon/Cargo.toml -- --check
cargo test --all-targets --locked --manifest-path lexicon/Cargo.toml
cargo clippy --manifest-path lexicon/Cargo.toml --all-targets -- -D warnings
```

Lexicon CLI:

```text
cargo fmt --manifest-path lexicon-cli/Cargo.toml -- --check
cargo test --all-targets --locked --manifest-path lexicon-cli/Cargo.toml
cargo clippy --manifest-path lexicon-cli/Cargo.toml --all-targets -- -D warnings
```

Migration parity fixtures and the pinned Go oracle are documented in [Rust migration](RUST_MIGRATION.md). When changing a migrated language adapter, run its Rust parity tests. When changing a still-transitional runtime/oracle path, also run the owning legacy suite.

## Complete repository verification

The canonical bounded root suite is:

```text
python scripts/workflow.py test
```

It validates Pitlord policy, documentation governance, the remaining Go/oracle suites, C#/Java/Kotlin integration paths, and the Rust Lexicon, Lexicon CLI, and Arcana crates.

Use `--jobs N` only when additional concurrency is intentional:

```text
python scripts/workflow.py test --jobs 2
```

## Adapter development

For a new language adapter or a substantial semantic expansion, start with [Adapter authoring](ADAPTER_AUTHORING.md). New long-lived first-party adapter work targets the native Rust `LanguageAdapter` contract. If the adapter must also work in the currently recommended Go runtime, implement the legacy executable/facts-v1 compatibility boundary described there.

## Semantic acceptance

Parser completion or nonzero output is not sufficient. Semantic changes must preserve:

- deterministic stable identities;
- canonical ordering;
- definite versus possible relationships;
- explicit unresolved evidence rather than invented targets;
- source ownership and spans;
- positive fixtures for promised behavior;
- negative gates for relationships that must not be emitted;
- deterministic output across repeat runs.

The detailed semantic contract is [Semantic acceptance](SEMANTIC_ACCEPTANCE.md). Versioned exchange/storage contracts live under [`spec/`](../spec/README.md).

## Rust migration workflow

The migration rule is parity first:

1. identify the current legacy/oracle behavior for the slice;
2. port ownership into the Rust `LanguageAdapter`/library boundary;
3. compare deterministic observable output;
4. keep the legacy implementation only while it still owns untranslated behavior or serves as the explicit oracle;
5. remove production dependence on the old path once its parity gate is satisfied;
6. update [Rust migration](RUST_MIGRATION.md), [Status](STATUS.md), and the owning adapter documentation.

Do not add a compatibility bridge merely to preserve obsolete process/JSONL mechanics. The Rust adapter contract returns typed facts directly.

## Repository smoke path

After a CLI, scan, storage, consumer, or adapter change, exercise a real repository:

```text
build/bin/lexicon init --repo /path/to/repository
build/bin/lexicon status --repo /path/to/repository
build/bin/lexicon doctor --repo /path/to/repository
build/bin/lexicon scan --repo /path/to/repository
```

Use `find`, `show`, `refs`, and `calls` to verify semantic lookup behavior when affected.

## Documentation checks

From the shared repository root:

```text
python .standards/docs_policy/check.py --repo .
python .standards/docs_policy/check.py --repo . --config docs-standard.lexicon.json
python .standards/docs_policy/check.py --repo . --config docs-standard.arcana.json
python scripts/check_docs.py
```

Update documentation in the same change when ownership, commands, state, contracts, adapters, performance guidance, or supported behavior change. Operator-facing docs should continue to recommend the pinned Go runtime until the Rust optimization gap is explicitly closed; implementation-facing migration docs should point to the current Rust owner where appropriate.

## Release packaging

Build a disposable distribution:

```text
python scripts/workflow.py build --version <version>
```

Create release archives and checksums:

```text
python scripts/workflow.py release --version <version>
```

See [Release packaging](RELEASE_PACKAGING.md) and the shared [installation guide](../../docs/reference/installation.md).

## Code map

| Concern | Current owner | Verification |
| --- | --- | --- |
| Reusable Lexicon library | `src/` | `tests/`, module tests |
| CLI host | `../lexicon-cli/src/` | `../lexicon-cli/tests/`, module tests |
| Native adapters | `src/adapters/` | adapter-specific Rust parity tests |
| Legacy/parity adapter sources | `adapters/` | owning oracle/runtime suites while migration remains |
| Migration comparator | `evaluation/rust_migration/` | pinned fixture comparisons |
| Versioned contracts | `spec/` | contract/golden tests |
| Root build/test/release composition | `../scripts/workflow.py` | `../scripts/test_workflow.py` |
| Documentation policy | `../.standards/docs_policy/`, `../docs-standard.lexicon.json` | documentation checks above |

## Related docs

- [Operator how-to](HOWTO.md)
- [Lexicon architecture](ARCHITECTURE.md)
- [Rust migration](RUST_MIGRATION.md)
- [Status](STATUS.md)
- [Semantic acceptance](SEMANTIC_ACCEPTANCE.md)
- [Release packaging](RELEASE_PACKAGING.md)

## Notes

The presence of a legacy Go or external adapter source tree does not by itself make that path the current product owner. Use [Rust migration](RUST_MIGRATION.md) and the current source tree to determine ownership.
