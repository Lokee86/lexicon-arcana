# Lexicon development and verification

Parent index: [Lexicon Documentation](README.md)

## Purpose

This document defines Lexicon's build, focused-test, complete-test, semantic-validation, adapter-development, contract-change, documentation, smoke-test, and packaging workflows.

## Overview

Verification combines application tests, independently owned adapter suites, real-repository corpora, contract validation, smoke operations, and release-layout checks.

This document defines the supported source-development workflow and the minimum verification expected for changes.

## Prerequisites

The complete repository uses several runtimes because each language adapter is self-contained:

- Go 1.26 or newer plus a working CGO C compiler for the application, C/C++ adapter, Go adapter, GDScript adapter, LotusScript adapter, and generic adapter;
- Python 3 with `pytest` for the Python adapter and evaluation tools;
- Ruby for the Ruby adapter;
- Rust and Cargo for the Rust adapter;
- JDK 21 or newer for Java compiler-backed analysis and Java release packaging;
- a .NET SDK for the C# Roslyn/MSBuild adapter;
- Node.js and npm for the JavaScript, TypeScript, and Svelte adapter.

A focused change only requires the runtimes used by the affected component. A full release or complete test matrix requires all of them.

## Build the application

From the repository root:

```text
go build -o bin/lexicon ./cmd/lexicon
```

For source-tree execution, point initialization at this checkout's adapter directory:

```text
bin/lexicon init --repo /path/to/repository --adapters ./adapters
```

`LEXICON_ADAPTERS` may be used instead of repeating `--adapters`.

## Focused test commands

### Application

```text
go test ./...
go test -race ./...
```

### C and C++ adapter

```text
cd adapters/c-family
go test ./...
go test -race ./...
```

### Go adapter

```text
cd adapters/go
go test ./...
go test -race ./...
```

### GDScript adapter

```text
cd adapters/gdscript
go test ./...
```

### C# adapter

```text
python adapters/csharp/tests/test_adapter.py
```

This script builds the Roslyn adapter, verifies file-mode and restored MSBuild project analysis, and checks determinism and incremental output.

### Java adapter

```text
cd adapters/java
go test ./...
```

Set `LEXICON_JDK_HOME` when the JDK is not available through `JAVA_HOME`, `PATH`, or the repository-local `.tools/jdk` location.

### Kotlin adapter

```text
cd adapters/kotlin
go test ./...
```

### LotusScript adapter

```text
cd adapters/lotusscript
go test ./...
```

### Python adapter

```text
cd adapters/python
python -m pytest
```

### Ruby adapter

```text
cd adapters/ruby
ruby test/test_adapter.rb
```

### Rust adapter

```text
cargo fmt --manifest-path adapters/rust/Cargo.toml -- --check
cargo test --manifest-path adapters/rust/Cargo.toml
cargo clippy --manifest-path adapters/rust/Cargo.toml --all-targets -- -D warnings
```

For Rust semantic-performance changes, also run a full rebuild on a representative real repository rather than relying only on fixtures. Dated evidence from 2026-08-07: rebuilding Arcana's 121 Rust source files (about 929 functions) completed in about 52 seconds after removing whole-AST/per-merge cloning from the fixed-point hot path; the prior path exceeded 180 seconds in the same local audit.

### JavaScript, TypeScript, and Svelte adapter

```text
npm --prefix adapters/typescript install
npm --prefix adapters/typescript run build
npm --prefix adapters/typescript test
```

## Complete test matrix

The canonical complete suite is:

```text
python evaluation/run_tests.py
```

It runs the root Go suite and every adapter suite with the repository's expected commands. It is the preferred final verification after cross-cutting changes.

Concurrency changes must also run both available race suites:

```text
go test -race ./...
go -C adapters/go test -race ./...
```

## Semantic acceptance

Parser completion or nonzero output is not sufficient. Semantic changes must satisfy the observable gates in [SEMANTIC_ACCEPTANCE.md](SEMANTIC_ACCEPTANCE.md).

At minimum, affected streams must verify:

- deterministic stable identities;
- canonical record and key ordering;
- correct definite versus possible relationships;
- explicit unresolved evidence instead of fabricated targets;
- source ownership and spans;
- positive fixture coverage for promised relationships;
- negative gates for relationships that must not be emitted;
- byte-identical repeated output.

Validate a generated adapter stream with:

```text
python tools/validate_jsonl.py /path/to/facts.jsonl
python tools/semantic_report.py /path/to/facts.jsonl
```

Compare repeat runs with:

```text
python evaluation/compare_jsonl.py LEFT.jsonl RIGHT.jsonl
```

## Real-repository corpus

Restore externally pinned corpus inputs when required:

```text
python evaluation/bootstrap_corpus.py
```

Run the full corpus:

```text
python evaluation/run_validation.py --jobs 3
```

Useful focused forms include:

```text
python evaluation/run_validation.py --adapter gdscript
python evaluation/run_validation.py --case gdscript-space-rocks-client --jobs 1
```

A complete passing run may replace `evaluation/validation/baseline.json`. Generated outputs under `evaluation/validation/generated/` are evidence artifacts and remain ignored by Git.

## Application smoke tests

The Python smoke tools exercise packaged application operations and runtime reconciliation:

```text
python tools/smoke_app.py
python tools/smoke_operations.py
python tools/test_reconcile_runtime.py
python tools/test_semantic_report.py
```

Use them when changing CLI operations, snapshot publication, export, consumers, garbage collection, or runtime-evidence tooling.

## Adding or expanding an adapter

A language adapter belongs under `adapters/<language>/` and remains executable without importing application internals.

Required work includes:

1. define the language's canonical identities while using common facts-v1 kinds and relations where semantics align;
2. implement deterministic discovery and permanent exclusions;
3. emit complete facts-v1 streams before adding incremental scope support;
4. preserve file ownership for every replaceable record;
5. classify unsupported forms explicitly;
6. add focused declaration, relationship, dataflow, dependency, unresolved, and determinism fixtures;
7. register the language with the application runner and language registry;
8. document setup, modeled semantics, canonical identities, conservative boundaries, dependency behavior, dataflow behavior, and tests in the adapter README;
9. add representative corpus coverage when a suitable repository exists;
10. update [STATUS.md](STATUS.md), [adapters/README.md](../adapters/README.md), and affected contracts.

Adapters must not introduce Arcana, higher-level discovery/workflow, documentation-policy, or other consumer-specific behavior.

## Contract changes

Changes to `spec/` require explicit compatibility analysis. Do not silently change record meaning, identity payloads, ownership rules, sorting, binary object bytes, manifest hashing, or runtime-evidence reconciliation.

A contract change must include:

- specification updates;
- encoder and decoder changes;
- validator updates;
- golden or compatibility fixtures;
- migration behavior where existing snapshots or objects remain readable;
- consumer coordination when the change crosses repository boundaries.

See [spec/README.md](../spec/README.md).

## Documentation checks

Documentation is part of the change, not a later cleanup task.

Before completion:

- update the root README when entry-point behavior or support changes;
- update `docs/APPLICATION.md` for CLI, state, or operational changes;
- update `docs/ARCHITECTURE.md` for ownership, lifecycle, concurrency, or storage changes;
- update `docs/STATUS.md` for capability or limitation changes;
- update the owning adapter README for language behavior;
- update `spec/` for normative format changes;
- mark measurements as dated evidence rather than timeless current performance;
- verify every new document is linked from the appropriate folder index.

## Release packaging

Build a distribution with:

```text
python tools/package_release.py --output release --version <version>
```

Omit `--version` only for local packaging tests where `lexicon version` may report `dev`. Then run the packaged smoke path against a temporary repository before publishing. Distribution contents and runtime requirements are documented in [RELEASE_PACKAGING.md](RELEASE_PACKAGING.md).

## Code map

| Development concern | Primary implementation or artifact | Related verification |
| --- | --- | --- |
| Application build and tests | root `go.mod`, `cmd/`, `internal/` | package-local Go tests |
| Adapter test matrix | `evaluation/run_tests.py` | all language adapter suites, including C#, Java, and Kotlin |
| Real-repository corpus validation | `evaluation/run_validation.py`, `corpus.json`, `validation/` | repeat-run summaries and baselines |
| Smoke tests and operations | `tools/smoke_app.py`, `smoke_operations.py`, `smoke_installers.py` | packaged application checks |
| Semantic report tooling | `tools/semantic_report.py`, `semantic_depth.py`, `call_resolution_metrics.py` | corresponding tool tests |
| Release packaging | `tools/package_release.py` | `tools/test_package_release.py` |
| Adapter contracts | `spec/`, `internal/objectstore/analysis.go`, `ingest_parse.go`, `records.go` | contract and object-store tests |
| Documentation | `docs/`, adapter READMEs, root documentation checker | root and component documentation gates |

Generated validation output and packaged binaries are evidence or build products, not source owners.

## Related docs

- [Lexicon architecture](ARCHITECTURE.md)
- [Semantic acceptance gates](SEMANTIC_ACCEPTANCE.md)
- [Semantic corpus validation](SEMANTIC_CORPUS_VALIDATION.md)
- [Release packaging](RELEASE_PACKAGING.md)

## Notes

Generated validation results and packaged artifacts are evidence or build outputs, not implementation owners.
