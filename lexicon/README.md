# Lexicon

> **Canonical source:** Lexicon lives under `lexicon/` in the shared `Lokee86/lexicon-arcana` repository. Lexicon remains an independently buildable application, adapter platform, snapshot producer, and reusable language-analysis engine.

Lexicon is the lead semantic-analysis product in the Lexicon + Arcana stack and a deterministic analysis provider for Warlock and other consumers. It turns source repositories into deterministic, versioned facts about files, symbols, calls, dataflow, inheritance, dependencies, and unresolved relationships.

Lexicon is primarily a one-shot CLI application. It can also run an optional filesystem watch mode through `lexicon demon`, but consumers do not depend on a resident Lexicon process.

## Current state

Lexicon currently provides:

- deterministic adapters for C, C++, Go, GDScript, LotusScript, Python, Ruby, Rust, JavaScript, TypeScript, and Svelte script blocks;
- conservative generic fallback coverage for curated source-code extensions without a dedicated adapter;
- a normalized facts-v1 JSONL adapter boundary;
- immutable content-addressed binary fact objects;
- atomic repository snapshots and crash-safe publication;
- source repositories need not use Git; Lexicon maintains its own private change-detection mirror and content identities;
- dependency-aware scoped analysis with complete-language fallback;
- concurrent language analysis under one process-wide CPU budget;
- adaptive parallel semantic resolution inside the Go adapter;
- repository-wide interstack tracing for HTTP boundaries, packet channels, and shared configuration keys;
- deterministic post-publication consumer hooks for tools such as Arcana;
- repeatable fixture and real-repository validation.

The adapters are functional semantic analyzers, not merely syntax inventories. Precision varies by language and dynamic behavior; unsupported relationships remain unresolved instead of being guessed. See [Current status and limits](docs/STATUS.md).

## Supported adapters

| Language surface | Implementation | Semantic frontend | Scope |
| --- | --- | --- | --- |
| C / C++ | Go | Official Tree-sitter C and C++ grammars | Mixed repositories, headers, declarations, includes, inheritance, calls, and conservative dataflow |
| Go | Go | `go/parser`, `go/types`, packages, SSA, and VTA | Multi-module repositories, typed calls, interfaces, dataflow, dependencies |
| GDScript | Go | Dedicated parser and bounded type-flow model | Godot projects, inheritance, callbacks, autoloads, local dispatch |
| LotusScript | Go | Dedicated parser, ODP/DXL extractor, and import-scoped script-library resolver | Classes and `Type` members, visibility, inheritance, typed calls, conservative reads/writes, ODP agents, and unresolved runtime targets |
| Python | Python | Standard-library `ast` | Imports, inheritance, protocols, higher-order flow, dataflow |
| Ruby | Ruby | Standard-library `Ripper` | Reopened types, mixins, blocks, Rails-aware bounded flow |
| Rust | Rust | `syn` and Cargo metadata | Workspaces, traits, implementations, callbacks, dependencies |
| JavaScript / TypeScript / Svelte | TypeScript | TypeScript compiler API and offset-preserving Svelte frontend | Typed and untyped JS/TS, JSX, CommonJS, Svelte script blocks |
| Other curated source extensions | Go | Conservative generic fallback | File/module facts, high-confidence declarations, static import evidence |

Adapter-specific behavior and limits are indexed in [adapters/README.md](adapters/README.md).

## Quick start

Build the application from the repository root:

```text
go build -o bin/lexicon ./cmd/lexicon
```

Initialize a source repository using this checkout's adapters:

```text
bin/lexicon init --repo /path/to/repository --adapters ./adapters
bin/lexicon status --repo /path/to/repository
```

Subsequent scans reuse unchanged immutable objects and narrow analysis when the previous snapshot makes that safe:

```text
bin/lexicon scan --repo /path/to/repository
bin/lexicon export --repo /path/to/repository --output /path/to/export
```

A packaged release places the executable beside its adapter directory, so `--adapters` is normally unnecessary. Run `install.ps1` on Windows or `install.sh` on Unix-like systems to install the complete extracted package for the current user. Source checkouts can also set `LEXICON_ADAPTERS`.

## Commands

| Command | Responsibility |
| --- | --- |
| `init` | Create `.lexicon/`, detect enabled languages, perform the first scan, and publish the initial snapshot |
| `scan` | Reconcile current source content and publish a new snapshot when analysis changes |
| `demon` | Watch the repository and invoke the same scan transaction after debounced changes |
| `rebuild` | Force complete analysis for all or selected enabled languages |
| `languages` | Inspect or change the enabled language set |
| `status` | Report repository, snapshot, language, and consumer state |
| `doctor` | Validate configuration, storage, adapters, runtimes, and consumers |
| `export` | Reconstruct verified deterministic JSONL libraries from an immutable snapshot |
| `gc` | Remove unreachable snapshots and fact objects while respecting retention and consumer pins |
| `consumer` | List, register, remove, or invoke deterministic post-publication consumers |
| `version` | Print the application version |

The complete flag reference and state layout are in [docs/APPLICATION.md](docs/APPLICATION.md).

## Architecture

Lexicon has five explicit ownership layers:

1. **Adapters** discover language-specific facts and emit facts-v1 JSONL.
2. **Scan orchestration** selects complete or scoped analysis, schedules adapters, and validates their output.
3. **Interstack resolution** derives conservative cross-language contract nodes and edges from the candidate language facts.
4. **Object storage** partitions facts by source owner, writes immutable binary objects, and publishes atomic manifests.
5. **Consumers** resolve `CURRENT` and read only immutable snapshot data.

JSONL is the stable adapter, export, and diagnostic boundary. Normal application scans parse each adapter stream once, then store compact binary fact objects without maintaining complete materialized JSONL libraries.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for ownership, transaction, incremental, concurrency, and recovery details.

## Contracts

The versioned public contracts are:

- [facts-v1](spec/facts-v1.md): adapter records, stable IDs, ownership, sorting, and incremental semantics;
- [objects-v1](spec/objects-v1.md): deterministic binary fact-object encoding;
- [snapshots-v1](spec/snapshots-v1.md): immutable manifests and atomic publication;
- [runtime-evidence-v1](spec/runtime-evidence-v1.md): optional run-specific observations that never rewrite static facts.

Contract governance and compatibility rules are in [spec/README.md](spec/README.md).

## Concurrency and determinism

Independent language adapters may execute concurrently under a weighted process-wide CPU budget. The Go adapter additionally partitions typed call and dataflow work into logical shards, processes those shards with a bounded worker pool, and merges shard-local facts through a deterministic reduction tree before the repository-wide SSA/VTA pass.

Logical shard count is separate from active worker count. Large repositories may have many logical partitions without launching an equivalent number of processes or goroutines. `LEXICON_MAX_WORKERS` can lower the worker ceiling for a machine or CI environment. Output must remain byte-identical across worker counts and merge shapes.

## Boundaries

Lexicon owns language extraction, normalized fact identities, source ownership, immutable analysis objects, and snapshot publication.

Lexicon does not own:

- graph query algorithms or packed graph traversal; Arcana owns those concerns;
- repository discovery policy, source/document ranking, investigation sessions, or a replacement Grimoire wrapper; those retired concerns have no Lexicon owner;
- agent/task/context orchestration; Warlock or another consumer owns that higher-level workflow;
- documentation policy or repository documentation repair; Demon Docs owns those concerns;
- runtime instrumentation providers; the runtime-evidence contract only defines their exchange boundary;
- general repository version control; the private Git mirror is an internal change detector, not a user-facing history.

## Documentation

- [Documentation index and rules](docs/README.md)
- [Application and operations](docs/APPLICATION.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Maintainer map](docs/MAINTAINER_MAP.md): routes common changes to canonical documents and implementation boundaries.
- [Development and verification](docs/DEVELOPMENT.md)
- [Current status and limits](docs/STATUS.md)
- [Semantic acceptance gates](docs/SEMANTIC_ACCEPTANCE.md)
- [Cross-adapter corpus validation](docs/SEMANTIC_CORPUS_VALIDATION.md)
- [Release packaging](docs/RELEASE_PACKAGING.md)
- [Evaluation harness](evaluation/README.md)

## Development

Run the complete application and adapter test matrix:

```text
python evaluation/run_tests.py
```

Run the real-repository semantic corpus:

```text
python evaluation/bootstrap_corpus.py
python evaluation/run_validation.py --jobs 3
```

Detailed prerequisites, focused commands, race checks, validation rules, and documentation requirements are in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

## Grimoire and the Warlock toolchain

Lexicon is independently usable, but its canonical source now shares the Grimoire repository with Arcana and Grimoire's discovery application. Arcana consumes Lexicon snapshots to build queryable graphs; Grimoire and other Warlock tools can consume the same facts without maintaining duplicate language adapters. Repository consolidation does not make Lexicon depend on either downstream component.

## License

Lexicon is available under the [PolyForm Shield License 1.0.0](LICENSE). Competing products and services require a separate commercial license. See the repository root [LICENSING.md](../LICENSING.md).
