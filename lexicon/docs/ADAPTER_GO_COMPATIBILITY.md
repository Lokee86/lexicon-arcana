# Go adapter compatibility

Parent index: [Lexicon Documentation](README.md)

Primary guide: [Adapter authoring](ADAPTER_AUTHORING.md)

## Purpose

Use this page only when a new adapter must run in the optimized Go Lexicon currently recommended to operators.

The pinned Go reference revision is:

```text
758af9daf6e71fc0a7ebb837875efe366f6403fd
```

New long-lived first-party adapter ownership should still target the native Rust contract first.

## Overview

The Go runtime executes language adapters as processes and consumes facts-v1 JSONL.

A compatible adapter therefore needs:

- a standalone executable or runtime entry point;
- facts-v1 output;
- Go language-registry integration;
- Go runner/packaging integration;
- the same semantic acceptance and determinism guarantees as a native adapter.

This is a compatibility boundary, not the future Rust architecture.

## Process contract

The Go runner passes:

```text
--repo <repository>
--output <path-or-dash>
--changed-file <path>   # repeatable
--removed-file <path>   # repeatable
```

Adapters marked for partitioned execution may also receive:

```text
--workers N
--shards N
--merge-fan-in N
```

Support `--output -` for stdout when practical.

A full run emits one complete facts-v1 stream. An incremental run must follow the ownership/removal rules in [facts-v1](../spec/facts-v1.md).

## Register the language

Add the language to:

```text
lexicon/internal/languages/registry.go
```

Define:

- canonical language name;
- adapter directory;
- source extensions;
- relevant config files;
- whether partitioned execution is supported;
- whether streaming output is supported.

Language detection and scan planning use this registry.

## Add runner support

The Go process launcher lives under:

```text
lexicon/internal/adapters/
```

`runner.go` selects the execution method for each language. Existing adapters demonstrate several supported patterns:

- Go executable / `go run`;
- .NET executable / `dotnet run`;
- Python module;
- Ruby script;
- Cargo executable;
- Node.js compiled JavaScript.

Choose the runtime that belongs naturally to the language analyzer. Do not add another runtime merely for consistency.

## Packaged executable convention

Native packaged executables use:

```text
adapters/<language>/lexicon-<language>
```

with `.exe` on Windows.

If the adapter needs additional runtime files, ensure the release workflow copies only production assets and excludes tests, caches, generated output, and local build trees.

## Output contract

The first line is the facts-v1 header. Remaining lines are canonical node, edge, and unresolved records.

The adapter must:

- normalize paths;
- use stable IDs;
- preserve ownership;
- sort records canonically;
- distinguish definite and possible relationships;
- emit explicit unresolved evidence;
- produce byte-identical output for identical inputs.

Validate output with the existing Lexicon fact validators and semantic reports.

## Fingerprinting

The Go runtime fingerprints adapter implementation files beneath the adapter directory while excluding common tests/build/cache directories.

Behavior-affecting production source must therefore live inside the adapter's owned directory or otherwise be included deliberately in the fingerprint boundary.

A changed implementation fingerprint can force reanalysis.

## Full before incremental

Implement and validate full output first.

Only then add changed/removed-file handling. If a scoped run cannot preserve sound ownership or semantic context, emit/trigger the full-analysis path instead of returning incomplete certainty.

## Compatibility tests

In addition to the normal adapter acceptance suite, verify:

- the Go registry detects the language from owned extensions/config files;
- the runner launches the correct runtime;
- file and stdout output modes behave as documented;
- packaged executable discovery works when applicable;
- adapter fingerprints change when behavior-affecting source changes;
- a Go Lexicon scan accepts the emitted facts;
- Arcana can consume the resulting published Lexicon snapshot.

## Removal condition

Do not preserve this process boundary after the adapter is fully native in Rust merely for internal compatibility.

Once the Rust adapter reaches the required parity and performance gates and the Go runtime is no longer the operator recommendation, the legacy process implementation should be removable without changing facts-v1 semantics.

## Related docs

- [Adapter authoring](ADAPTER_AUTHORING.md)
- [facts-v1](../spec/facts-v1.md)
- [Rust migration](RUST_MIGRATION.md)
- [Semantic acceptance](SEMANTIC_ACCEPTANCE.md)
- [Release packaging](RELEASE_PACKAGING.md)

## Notes

The Go compatibility layer exists to support the current optimized runtime. It should not constrain the native Rust adapter design.
