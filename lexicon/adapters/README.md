# Lexicon language adapters

Lexicon is currently migrating adapter ownership into native Rust implementations under `lexicon/src/adapters/`. This `lexicon/adapters/` directory contains the legacy executable implementations, parity oracles, and Go-runtime compatibility assets that remain necessary during that migration.

Both boundaries implement the same language semantics and facts-v1 meaning; they differ in execution mechanics.

## Purpose

Adapters translate language syntax and static semantic evidence into normalized repository facts:

- source structure and declarations;
- imports and dependencies;
- containment and definition ownership;
- inheritance, implementation, trait, mixin, and override relationships;
- definite calls and conservative possible calls;
- reads and writes where local dataflow is sound;
- explicit unresolved evidence for unsupported, ambiguous, dynamic, external, or built-in targets.

## Does not own

Adapters do not own:

- Lexicon repository initialization or CLI behavior;
- incremental plan selection or scoped-repository construction;
- snapshot manifests, object storage, publication, recovery, or garbage collection;
- Arcana graph storage or query policy;
- Grimoire ranking or context-package construction;
- consumer-specific interpretation of facts.

Those boundaries keep one language implementation reusable by every Warlock consumer.

## Direct folders

| Folder | Language surface | Implementation | Primary semantic frontend |
| --- | --- | --- | --- |
| [c-family/](c-family/README.md) | C and C++ | Go | Official Tree-sitter C and C++ grammars |
| [go/](go/README.md) | Go | Go | `go/parser`, `go/types`, packages, SSA, and VTA |
| [gdscript/](gdscript/README.md) | GDScript | Go | Dedicated parser and bounded type-flow model |
| [csharp/](csharp/README.md) | C# | C# | Roslyn compiler APIs with optional MSBuild project loading |
| [java/](java/README.md) | Java | Go + Java | Deterministic parser plus compiler-backed `jdk.compiler` attribution |
| [kotlin/](kotlin/README.md) | Kotlin | Go | Dedicated structural and bounded semantic resolver |
| [generic/](generic/README.md) | Curated unsupported source extensions | Go | Conservative line-oriented fallback |
| [lotusscript/](lotusscript/README.md) | LotusScript | Go | Dedicated parser, ODP/DXL extractor, import-scoped resolution, and conservative dataflow |
| [python/](python/README.md) | Python | Python | Standard-library `ast` |
| [ruby/](ruby/README.md) | Ruby | Ruby | Standard-library `Ripper` |
| [rust/](rust/README.md) | Rust | Rust | `syn` and Cargo metadata |
| [typescript/](typescript/README.md) | JavaScript, TypeScript, and Svelte script blocks | TypeScript | TypeScript compiler API and Svelte script extraction |

## Shared execution contract

Every adapter must:

- accept a repository root and output destination;
- support `-` or the documented equivalent for stdout when practical;
- emit exactly one facts-v1 header followed by canonical nodes, edges, and unresolved records;
- use normalized repository-relative forward-slash paths;
- exclude Git/worktree metadata, Warlock state directories, dependencies, build outputs, and language caches;
- use stable SHA-256 identities without absolute checkout paths;
- preserve definite versus possible relationship semantics;
- retain source ownership for records that may be incrementally replaced;
- produce byte-identical output for identical input and configuration;
- avoid executing analyzed application code or manifests;
- emit a full stream when file ownership cannot be determined safely.

Incremental-capable adapters additionally accept changed and removed file scopes defined by [facts-v1](../spec/facts-v1.md). Shared synthetic records may replace the complete shared set only when the stream declares `shared_complete: true`.

## Relationship policy

A `calls` edge means one definite statically identified callable contract. Multiple defensible concrete runtime targets use `possible-calls`. Interface, trait, protocol, and mixin declarations remain contracts unless the language provides separate runtime evidence.

Adapters must not select an arbitrary same-named declaration to avoid an unresolved record. Unsupported or ambiguous evidence remains explicit.

`imports` records source-level import evidence. `depends-on` records package, module, plugin, resource, or manifest dependency evidence. One does not replace the other.

## Parallelism

The application may execute independent adapters concurrently. An adapter must therefore keep all mutable execution state process-local and must not write shared repository files outside its requested output.

The Go adapter also accepts worker, logical-shard, and merge-fan-in parameters. Its shard-local semantic work must merge deterministically and produce the same facts for every valid execution plan.

Other adapters may add safe process or project partitioning later, but partition boundaries must preserve language semantics and deterministic ownership. Arbitrary file sharding is not acceptable when it changes the available type, import, or dispatch context.

## Adding an adapter

Start with the dedicated [adapter authoring guide](../docs/ADAPTER_AUTHORING.md).

New first-party adapters target the native Rust `LanguageAdapter` contract under `../src/adapters/<language>/`. If the adapter must also run in the currently recommended optimized Go Lexicon, add the separate executable compatibility boundary described in [Go adapter compatibility](../docs/ADAPTER_GO_COMPATIBILITY.md).

Every new adapter still requires explicit identities, exclusions, semantic fixtures, unresolved-evidence behavior, deterministic output, full-analysis support before incremental narrowing, an adapter README, acceptance evidence, and status/index updates.

## Placement rules

Native Rust parser, resolver, model, emitter, fixtures, and tests belong under `src/adapters/<language>/`.

Legacy executable/parity-oracle code needed by the Go runtime belongs under `adapters/<language>/`.

Cross-language record meaning belongs in `spec/`. Shared scan/storage orchestration belongs outside adapters. Do not create a cross-runtime helper dependency merely to share implementation convenience; share behavior through versioned contracts and acceptance fixtures.

## Code map

| Adapter boundary | Primary implementation | Related tests |
| --- | --- | --- |
| Supported language registry | `internal/languages/registry.go` | language registry tests |
| Adapter discovery, fingerprints, and execution | `internal/adapters/registry.go`, `runner.go`, `runner_packaged.go` | adapter runner/registry tests |
| Shared facts contract | `spec/facts-v1.md` | object-store contract tests |
| C/C++ | `adapters/c-family/` | package-local Go tests |
| Go | `adapters/go/` | package-local Go tests |
| GDScript | `adapters/gdscript/` | package-local Go tests |
| C# | `adapters/csharp/` | .NET adapter tests |
| Java | `adapters/java/` | package-local Go tests plus embedded compiler fixtures |
| Kotlin | `adapters/kotlin/` | package-local Go tests |
| Generic fallback | `adapters/generic/` | package-local Go tests |
| LotusScript | `adapters/lotusscript/` | package-local Go tests |
| Python | `adapters/python/lexicon_python/` | `adapters/python/tests/` |
| Ruby | `adapters/ruby/` | `adapters/ruby/test/` |
| Rust | `adapters/rust/src/` | Rust module tests and fixtures |
| JavaScript/TypeScript/Svelte | `adapters/typescript/src/` | `adapters/typescript/tests/` |

Adapters own language discovery and semantics only. Snapshot publication, cross-language interstack resolution, and graph storage belong elsewhere.
