# Adapter authoring guide

Parent index: [Lexicon Documentation](README.md)

## Purpose

Use this guide to add a new language adapter or substantially expand an existing one.

It explains the native Rust adapter boundary, semantic expectations, registration, testing, and acceptance. For compatibility with the currently recommended Go runtime, see [Go adapter compatibility](ADAPTER_GO_COMPATIBILITY.md).

## Overview

An adapter converts repository source into deterministic semantic evidence:

```text
repository source
  -> discovery + parser/compiler frontend
  -> declarations + semantic resolution
  -> nodes / edges / unresolved records
  -> Lexicon validation + immutable snapshot
```

Adapters own language semantics. They do not own snapshot storage, Arcana graph behavior, ranking, agent orchestration, or documentation policy.

## Choose the target

New first-party adapter work should target the native Rust `LanguageAdapter` contract under:

```text
lexicon/src/adapters/<language>/
```

The Rust host accepts typed `Analysis` directly. Do not create a subprocess or JSONL bridge for new Rust-native adapters.

The optimized Go Lexicon at `758af9daf6e71fc0a7ebb837875efe366f6403fd` remains the recommended operator runtime until Rust optimization catches up. If a new adapter must work there immediately, implement the additional compatibility boundary in [ADAPTER_GO_COMPATIBILITY.md](ADAPTER_GO_COMPATIBILITY.md).

There is currently no drop-in third-party adapter directory. Adding a supported language requires registering it in Lexicon source.

## Define the semantic boundary first

Before choosing a parser, write down:

- canonical language name;
- owned source extensions;
- project/config files affecting discovery;
- declarations the adapter promises to model;
- relationships it can prove;
- dynamic/unsupported forms that remain unresolved;
- canonical identity rules;
- minimum safe analysis unit: file, package, crate, project, translation unit, or repository.

Tree-sitter, compiler APIs, standard-library parsers, and custom parsers are all acceptable. The contract is semantic evidence, not parser choice.

## Implement full analysis first

Start with complete repository analysis:

1. discover owned source deterministically;
2. parse the complete required semantic scope;
3. emit stable declarations and ownership;
4. resolve only relationships the evidence supports;
5. emit unresolved evidence for everything else;
6. produce identical facts for identical source/configuration.

Add incremental narrowing only after full analysis is correct.

## Native Rust contract

Implement:

```rust
pub trait LanguageAdapter: Send + Sync {
    fn implementation_version(&self) -> &'static str;
    fn implementation_fingerprint(&self) -> String;
    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError>;
}
```

`AdapterRequest` supplies the language, full/incremental mode, repository, changed/removed files, and optional execution limits.

Return typed `Analysis`. The host then validates contract versions, restricts incremental ownership, canonicalizes ordering, validates facts, and rejects output for the wrong language.

Use `src/adapters/generic/` as the smallest structural example and `src/adapters/python/` as a fuller semantic example.

## Register the adapter

A native language normally requires:

1. `src/adapters/<language>/`;
2. module export in `src/adapters/mod.rs`;
3. a `LanguageDefinition` in `src/languages/definition.rs`;
4. registration in `AdapterHost::new`;
5. parser/compiler dependencies in `lexicon/Cargo.toml` when needed;
6. focused fixtures/tests;
7. adapter README, adapter index, and status updates.

Do not register an adapter merely to make language detection succeed. Registration means Lexicon may select it for real scans.

## Version and fingerprint

Every adapter provides a human-readable implementation version and deterministic fingerprint.

Use `source_fingerprint` over all source files that can change adapter behavior when practical. A semantic behavior change must change the fingerprint so Lexicon can invalidate stale analysis safely.

Never fingerprint absolute checkout paths, timestamps, caches, or machine-specific state.

## Facts and identities

Follow [facts-v1](../spec/facts-v1.md).

Stable node IDs derive from:

```text
lexicon:v1\0<language>\0<kind>\0<canonical identity>
```

Canonical identities must survive different checkout paths, repeated scans, worker/shard changes, and unrelated repository edits.

Use normalized repository-relative forward-slash paths. Prefer semantic identities over raw source offsets.

## Relationship policy

Use `calls` only for one statically justified callable target.

Use `possible-calls` when several concrete runtime targets are defensible.

Preserve distinctions such as:

- `imports` vs `depends-on`;
- `extends` vs `implements`;
- `reads` vs `writes`;
- declaration contracts vs runtime implementation targets.

Never choose an arbitrary same-named symbol to avoid an unresolved record.

## Unresolved evidence

Unresolved records are valid semantic output.

Emit them for ambiguous, dynamic, external, built-in, generated, or unsupported targets. Preserve the source expression/span and a specific reason when possible.

A truthful unresolved edge is better than invented graph structure.

## Ownership and incremental safety

Every replaceable fact needs recoverable source ownership through `owner`, span, or source node.

For incremental output:

- emitted records belong to declared changed files;
- removed files emit no replacements;
- shared facts are replaced only with `shared_complete: true`;
- uncertain ownership/topology falls back to full analysis.

If the language's minimum sound unit is larger than a file, expand scope instead of pretending file-local analysis is safe.

## Determinism and parallelism

Identical source, configuration, schema, and adapter version must produce identical facts.

Therefore:

- sort discovery;
- canonicalize map/set iteration before emission;
- make candidate ordering deterministic;
- ignore worker completion order;
- keep mutable state local to one analysis;
- never mutate the analyzed repository;
- never execute analyzed application code or arbitrary manifests.

Partitioning is optional. Add it only when it preserves semantic context.

## Minimum tests

A usable adapter needs positive and negative fixtures for:

- discovery/exclusions;
- declarations and stable IDs;
- imports/dependencies;
- promised relationships;
- ambiguous/dynamic/external targets;
- unresolved evidence;
- reads/writes when claimed;
- inheritance/interfaces/traits when claimed;
- malformed/unsupported syntax;
- repeat-run determinism;
- full-analysis behavior.

When incremental analysis is added, test changed files, removals, shared facts, unsafe fallback, and equivalence with a full rebuild.

## Acceptance checklist

Before merge:

- language definition and host registration are correct;
- full analysis works before incremental optimization;
- identities contain no absolute checkout state;
- definite and possible relations remain distinct;
- unsupported relationships remain unresolved;
- repeated output is deterministic;
- facts validate against facts-v1;
- fixtures cover every claimed relationship family;
- representative real-repository validation exists when practical;
- adapter README documents setup, semantics, identities, dependencies, dataflow, limits, and tests;
- `STATUS.md` and the adapter index describe implemented behavior only.

## What does not belong in an adapter

Do not put Arcana traversal, architecture policy, ranking, snapshot publication, garbage collection, cross-language orchestration, documentation enforcement, repository mutation, or probabilistic guesses into a language adapter.

The adapter's job is narrow: produce deterministic semantic evidence justified by the supplied repository state.

## Related docs

- [Go adapter compatibility](ADAPTER_GO_COMPATIBILITY.md)
- [Adapter index](../adapters/README.md)
- [facts-v1](../spec/facts-v1.md)
- [Semantic acceptance](SEMANTIC_ACCEPTANCE.md)
- [Dependency semantics](DEPENDENCY_SEMANTICS.md)
- [Development](DEVELOPMENT.md)
- [Rust migration](RUST_MIGRATION.md)
- [Contributing](../CONTRIBUTING.md)

## Notes

A smaller adapter with explicit unresolved evidence is preferable to a larger adapter that invents certainty.
