# Lexicon Rust adapter

This directory contains a self-contained Rust CLI that emits deterministic Lexicon facts v1 JSONL for Cargo repositories and workspaces.

## Usage

From this directory:

```text
cargo run -- --repo /path/to/repository --output /path/to/facts.jsonl
```

The adapter uses `cargo_metadata` for workspace and target discovery and `syn` for source parsing. `--repo` may point to a larger repository without a root `Cargo.toml`; the adapter discovers nested Cargo packages and workspaces while excluding generated, dependency, cache, worktree, and Warlock state directories.

The scanner excludes Git/worktree metadata, generated output, dependency trees, caches, and all Warlock tool-state directories.

## Analysis model

Adapter version 0.4.0 emits:

- repository, directory, file, crate/module, type, trait, function, method, import, and macro facts;
- inline and external module ownership;
- local imports, grouped imports, aliases, globs, and re-export bindings when their targets are statically unique;
- inherent and trait implementation relationships;
- `extends`-equivalent implementation ownership through `implements`, plus `overrides` from trait implementation methods to trait contract methods;
- free-function, associated-function, method, constructor-like, UFCS, and local macro call edges;
- receiver and return-value propagation through bindings, fields, parameters, and local expressions;
- callable propagation through function values, closures, callback parameters, tuples, and fields;
- definite `calls` edges and conservative `possible-calls` edges for generic or multi-target trait dispatch;
- explicit builtin, external, dynamic, missing, ambiguous, and unsupported classifications where a definite local target cannot be proven.

Canonical identities are based on Cargo package/target/module-qualified names and normalized repository-relative paths. Absolute checkout paths are never used in node identities or emitted paths.

Trait declarations are contracts and are never emitted as runtime call targets. A single proven concrete implementation emits `calls`; multiple concrete implementations or unconstrained generic/trait-object candidates emit `possible-calls`. Inherited/default trait methods resolve only through indexed repository-local implementations.

Semantic propagation uses a bounded fixed-point pass over the parsed function set. Passes reuse parsed function bodies rather than cloning complete ASTs, and `ValueSet` propagation detects in-place growth without cloning the prior value. Name and re-export resolution likewise walks only relevant qualified-name prefixes. These are scalability invariants: changing them can multiply whole-repository Rust analysis cost without changing emitted semantics.

## Conservative boundaries

The adapter performs static analysis only. It does not expand procedural macros, execute build scripts, infer runtime plugin registration, or guess targets created through unsafe pointer manipulation, reflection-like registries, or unconstrained dynamic dispatch.

External crates remain `external-target` unless their source is part of the scanned workspace. Macro-generated declarations that are not visible to `syn` cannot be indexed directly.
Dynamic dispatch remains unresolved for procedural-macro-generated methods, build-script registration, reflection-like registries, unconstrained runtime trait objects, unsafe pointer mutation, and other runtime-generated implementations.

## Verification

```text
cargo fmt -- --check
cargo test
cargo clippy --all-targets -- -D warnings
```

The semantic fixture suite covers declarations, imports, traits, inherent methods, field aliases, constructor-like calls, UFCS, local macros, callbacks, generic trait dispatch, canonical ordering, relative paths, unresolved classifications, and byte-identical repeat runs.

## Dependency semantics

Cargo normal dependencies, `[dev-dependencies]`, `[build-dependencies]`, target-conditioned dependency tables, and literal path dependencies emit deterministic `depends-on` facts. Dependency edges carry category, Cargo constraint, source, optional/dev/build/peer/path flags, and a target condition when present. External targets are facts-v1 `module` nodes with `dependency:rust:<normalized-target>` identity and `@dependencies/rust/...` paths; local path dependencies resolve to scanned package modules when available. Resolved local `use` imports additionally emit module-to-module local dependencies while preserving `imports`.

Malformed or dynamically generated Cargo metadata, procedural-macro-generated dependencies, runtime registration, and unresolved package contents are unsupported. The adapter uses Cargo metadata for manifest data but does not execute analyzed application code or build scripts.
## Dataflow facts

The adapter emits conservative `reads` and `writes` edges from functions and closures to repository-local parameters, locals, constants/statics, and resolved fields. Assignments write, compound assignments read and write, and initializer, argument, and return expressions contribute reads. Block shadowing is respected. Rust has no increment/decrement operators. Deref/alias analysis, unsafe mutation, external values, macro-generated bodies that are not parsed, and unresolved receiver fields are omitted rather than guessed.

## Code map

| Concern | Primary implementation | Related tests |
| --- | --- | --- |
| Entry and orchestration | `src/main.rs`, `cli.rs`, `orchestrator.rs` | `src/tests.rs` |
| Discovery, parsing, and extraction | `discovery.rs`, `parser.rs`, `extractor.rs`, `items.rs` | module tests and fixtures |
| Declarations and type resolution | `declarations.rs`, `semantic.rs`, `type_resolution.rs`, `resolve.rs` | module tests |
| Calls, relationships, and implementations | `call_*.rs`, `relationships.rs`, `implementations.rs` | module tests |
| Dataflow and dependencies | `dataflow.rs`, `flow.rs`, `dependencies.rs`, `imports.rs` | module tests and fixtures |
| Contract and output | `model.rs`, `contract.rs`, `emit.rs` | contract coverage in module tests |

Macro expansion and runtime dispatch remain conservative where source analysis cannot establish a unique target.
