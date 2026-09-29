# Adapter authoring guide

Parent index: [Lexicon Documentation](README.md)

## Purpose

Use this guide to add a new language adapter or substantially expand an existing one.

It explains the native Rust adapter boundary, semantic expectations, registration, testing, and acceptance. For compatibility with the currently recommended Go runtime, see [Go adapter compatibility](ADAPTER_GO_COMPATIBILITY.md).

## Overview

An adapter converts repository source into deterministic semantic evidence:

```text
repository source
  -> authoritative language frontend
  -> language-native semantic observations
  -> Lexicon normalization + materialization
  -> nodes / edges / unresolved records
  -> Lexicon validation + immutable snapshot
```

The language frontend owns authoritative syntax and compiler semantics. The Lexicon adapter owns translation into Lexicon's normalized semantic model, canonical identities, source ownership, relationship certainty, unresolved evidence, and deterministic materialization. It may add Lexicon-specific analysis over frontend evidence where the language implementation does not directly answer a Lexicon relationship question.

Adapters do not own snapshot storage, Arcana graph behavior, ranking, agent orchestration, or documentation policy.

## Choose the target

New first-party adapter work should expose the native Rust `LanguageAdapter` contract under:

```text
lexicon/src/adapters/<language>/
```

The Rust host accepts typed `Analysis` directly. That is the Lexicon-facing interface, not a requirement that parsing or compiler analysis run in Rust. When the authoritative frontend belongs to another runtime or toolchain, prefer a private language-native helper with a narrow, versioned observation protocol over reimplementing that frontend in Rust. The Go adapter's private semantic helper is the reference shape.

Do not use facts-v1 JSONL as the helper protocol. Helpers should return language-semantic observations; the Rust adapter remains responsible for Lexicon identities, facts materialization, ownership, validation, and publication integration.

### Canonical frontend seam

Keep the cross-process seam deliberately shallow:

```text
authoritative language frontend
  -> language-specific semantic observations
  -> Rust language adapter policy
  -> Analysis / facts-v1
```

Observation protocols are private to one language integration. Do not create a universal AST, universal compiler IR, cross-language observation enum, or second facts contract. A frontend may emit helper-local semantic keys for correlating observations, but those keys are not Lexicon node IDs and must carry enough evidence for the Rust adapter to construct canonical Lexicon identity.

The shared `src/adapters/frontend/` runner owns transport mechanics only: executable discovery, bounded request/response framing, stderr capture, protocol-version rejection, and transport performance metrics. It must not contain Go-specific or other language-specific semantic policy.

The Rust language adapter owns final relationship certainty, unresolved reasons, canonical identity, facts-v1 materialization, source ownership, deterministic ordering, and Lexicon-specific semantic extensions. A migrated adapter has one production frontend path: do not retain the replaced parser, a legacy protocol reader, a facts-v1 helper output mode, or a semantically weaker fallback merely to preserve the migration path.

The Go protocol v2 + `FrontendRunner` integration is the current reference implementation of this boundary.

The optimized Go Lexicon at `758af9daf6e71fc0a7ebb837875efe366f6403fd` remains the recommended operator runtime until Rust optimization catches up. If a new adapter must work there immediately, implement the additional compatibility boundary in [ADAPTER_GO_COMPATIBILITY.md](ADAPTER_GO_COMPATIBILITY.md).

There is currently no drop-in third-party adapter directory. Adding a supported language requires registering it in Lexicon source.

## Define the semantic boundary first

Before choosing the integration shape, write down:

- canonical language name;
- owned source extensions;
- authoritative language frontend and its runtime/toolchain requirements;
- project/config files affecting discovery;
- declarations the adapter promises to model;
- relationships the frontend can prove directly;
- Lexicon-specific analysis that must be layered over frontend evidence;
- dynamic/unsupported forms that remain unresolved;
- canonical identity rules;
- minimum safe analysis unit: file, package, crate, project, translation unit, or repository.

## Language-authoritative frontend invariant

Lexicon does not independently implement programming-language grammar or compiler semantics when an authoritative language frontend is available under usable terms.

Prefer, in order:

1. the language implementation's public compiler or analysis API;
2. the language implementation's official parser or semantic frontend;
3. a narrowly isolated integration with the language implementation's internal frontend when no stable public surface exists;
4. an established third-party parser only when the authoritative frontend is unavailable or impractical;
5. a Lexicon-owned parser only as a documented exception.

Open source alone does not require in-process coupling. If the authoritative frontend is unstable, runtime-specific, or awkward to embed, isolate it behind a private helper executable and a small versioned observation protocol. Toolchain availability should be reported explicitly rather than hidden behind a semantically weaker parser fallback.

A bespoke parser or compiler-semantic implementation requires documentation of why the authoritative frontend is unavailable or impractical. Convenience, implementation language, avoiding a helper process, or preserving an existing parser are not sufficient reasons.

Lexicon-specific semantic analysis may augment authoritative frontend output. It must not duplicate syntax, name binding, type resolution, overload selection, preprocessing, macro expansion, inheritance, or other compiler semantics merely to avoid integrating the language's own frontend.

The compatibility contract is Lexicon semantic evidence, canonical identity, ownership, determinism, and versioned facts—not the continued existence or exact mistakes of a particular parser implementation.

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
5. authoritative frontend integration, including private helper/runtime assets and explicit toolchain requirements when needed;
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
- malformed/unsupported syntax and authoritative-frontend diagnostics;
- missing/incompatible required toolchain behavior when the frontend is external;
- repeat-run determinism;
- full-analysis behavior.

When incremental analysis is added, test changed files, removals, shared facts, unsafe fallback, and equivalence with a full rebuild.

## Acceptance checklist

Before merge:

- language definition and host registration are correct;
- the adapter uses the authoritative language frontend, or the exception is explicitly documented with the concrete reason;
- any private helper protocol is narrow, versioned, and carries semantic observations rather than facts-v1 records;
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
