# Lexicon current status

Parent index: [Lexicon Documentation](README.md)

## Purpose

This document records Lexicon's current implemented capabilities, adapter coverage, incremental and parallel behavior, validation evidence, explicit limits, and documentation status.

## Overview

Status claims distinguish implemented behavior, measured evidence, and explicit non-claims. Future possibilities remain outside this document until implementation exists.

Status date: August 1, 2026.

This document describes the implementation currently present on `main`. Dated validation reports record evidence from specific runs and should not be treated as permanent performance guarantees.

## Application

Implemented:

- repository initialization and upward root discovery through `.lexicon/config.json`;
- configurable adapter discovery through `--adapters`, `LEXICON_ADAPTERS`, packaged adjacency, repository-local adapters, or the current working directory;
- detected and explicitly enabled language selection;
- complete scans and dependency-aware scoped scans;
- private source mirroring and Git-backed change detection;
- non-Git correctness through source content identities and snapshot comparison;
- immutable per-file and shared-language fact objects;
- deterministic binary v2 object encoding with binary v1 and legacy JSON-object reads;
- atomic snapshot manifests and `CURRENT` publication;
- durable `PENDING` recovery and single-writer locking;
- deterministic JSONL export;
- retention-aware object garbage collection with consumer pins;
- status and doctor diagnostics;
- deterministic post-publication consumers;
- optional debounced watch mode with periodic full reconciliation;
- concurrent language analysis under a weighted CPU budget.

The primary execution model remains one-shot CLI operations. Watch mode invokes the same bounded scan transaction and is not required by snapshot consumers.

## Adapter status

| Adapter | Version | Implemented semantic scope | Principal limits |
| --- | ---: | --- | --- |
| C / C++ | 0.5.0 | Shared C-family view, includer-aware parsing, include-closure translation units, bounded nested macro expansion with argument substitution and provenance, function-pointer flow, explainable call resolution, direct argument-to-parameter flow, arity and qualification pruning, direct receiver-type evidence, definite/possible calls, reads/writes | No full compiler/preprocessor replay, token pasting, stringification, variadic macro substitution, configuration-accurate branch evaluation, template instantiation, full overload ranking, ADL, virtual dispatch proof, generated headers, Objective-C, or CUDA semantics |
| Go | 0.1.0 | Multi-module discovery, packages, types, calls, closures, interfaces, implementations, overrides, dataflow, dependencies, SSA/VTA possible dispatch | Reflection, plugins, cgo/assembly, generated runtime behavior, and exact call-site graph retention |
| C# | 0.2.0 | Roslyn declarations and semantic relationships, optional restored MSBuild project graphs, project/package references, calls and candidate calls, inheritance, implementations, overrides, reads, writes, and deterministic full or scoped output | Project mode requires compatible restore assets; source generators, runtime reflection, application behavior, and exhaustive conditional configurations are not modeled |
| Java | 0.4.0 | Deterministic source and manifest analysis plus `jdk.compiler` attribution for repository-local calls, overrides, references, reads, and writes; bounded parallel source-root compilation; Maven and Gradle dependency evidence | External classpath/module-path symbols, annotation processing, generated-code expansion, complete build evaluation, reflection, and runtime dispatch remain unresolved |
| Kotlin | 0.4.0 | Deterministic source discovery, packages/imports, classes, interfaces, objects, companions, enums, members, properties, repository-local supertypes, extension calls, possible calls, overrides, dataflow, and literal Gradle/Maven dependency evidence | No Kotlin compiler or Gradle model, full inferred/safe/chained extension binding, source-set/plugin/generated-code semantics, delegated-property execution, reflection, or runtime dispatch |
| GDScript | 0.3.0 | Godot project scoping, classes, inheritance, autoloads, callbacks, bounded type flow, calls, possible calls, dataflow, dependencies | Scene-tree-only type evidence, engine internals, runtime script replacement, computed dispatch and resource paths |
| LotusScript | 0.3.0 | `.ls`/`.lss` discovery, Domino ODP `.lsa`/`.lsdb` extraction, script-library modules, transitive `Use` scope, visibility, classes and `Type` members, inheritance, definite typed calls, colon and `With` syntax, conservative reads/writes, and explicit unresolved targets | No generic `.txt`/`.bas`/`.vb` content detection, non-ASCII LMBCS raw-payload decoding, assignment-only type inference, implicit-variable modeling, alias/interprocedural flow, Domino design graph, complete runtime dispatch, or incremental narrowing |
| Python | 0.3.0 | Imports, lexical scopes, inheritance, protocols, callbacks, callable flow, C3 lookup, dataflow, dependencies | Monkey patching, metaclasses, dynamic imports/reflection, framework injection without ordinary value-flow evidence |
| Ruby | 0.3.0 | Reopened owners, inheritance, mixins, blocks, callbacks, bounded Rails-aware flow, dataflow, dependencies | Open runtime mutation, `send`/`eval`, refinements, dynamic constants, framework-generated behavior without declarations |
| Rust | 0.3.0 | Cargo workspaces, modules, traits, implementations, UFCS, callbacks, dataflow, dependencies | Procedural macro expansion, build-script-generated behavior, unsafe aliasing, unconstrained runtime registration |
| JavaScript / TypeScript / Svelte | 0.4.0 | Compiler-backed imports, inheritance, interfaces, calls, callbacks, CommonJS, JSDoc, Svelte script blocks, dataflow, dependencies | Svelte template semantics, Astro, prototype/runtime mutation, computed exports and properties, untyped external behavior |
| Generic fallback | 0.1.0 | Curated source extensions, file/module facts, high-confidence type and function declarations, static import evidence | No resolved calls, inheritance, dataflow, dispatch, or language-specific project semantics |

All adapters emit the same facts-v1 contract and preserve definite, possible, and unresolved relationships as distinct evidence.

## Incremental analysis

Implemented safe narrowing includes:

- reverse dependency closure from the previous snapshot;
- conservative inclusion of owners with unresolved relationships;
- forward dependency context in temporary scoped repositories;
- package expansion for Go and crate expansion for Rust;
- replacement of changed-file-owned objects only;
- unchanged object reuse;
- preservation of complete shared synthetic facts during partial scoped analysis;
- complete-language retry on unsafe topology, wrong stream mode, or scoped failure.

Current full-analysis triggers include additions, deletions, renames, copies, language configuration changes, adapter fingerprint changes, invalid prior state, and direct edits whose previous cross-file or unresolved relationships make partial replacement unsafe.

These fallbacks protect correctness. They are optimization limits, not contract failures.

## Parallel analysis

Implemented application-level behavior:

- independent language plans start concurrently;
- each plan reserves a weighted share of the process-wide CPU budget;
- results merge into the manifest in deterministic language-plan order.

Implemented partitioned-execution behavior currently includes the Go adapter:

- repository-size-dependent logical shards;
- bounded active workers;
- weighted work partitioning by semantic file size;
- shard-local nodes, edges, callsites, semantic identities, and unresolved state;
- deterministic fan-in merge;
- final repository-wide SSA/VTA pass;
Python now consumes the generic partitioned-execution contract with bounded process workers, deterministic file-local extraction, logical shard reduction, and repository-wide semantic resolution after merge. Go retains its language-specific typed SSA/VTA reconciliation. Deterministic output is required across worker counts, logical shard counts, and merge configurations.

## Go multi-module repositories

Implemented:

- discovery of every `go.mod` beneath the repository root;
- nearest-module ownership for each source file;
- independent package loading and semantic analysis per module;
- deterministic merge into one repository fact stream;
- module-qualified package and symbol identities;
- repository-directory identity when no root-level module exists.

This removes the previous assumption that a scanned repository contains exactly one root `go.mod`.

## Validation evidence

Current acceptance mechanisms include:

- application and adapter fixture suites;
- root and Go-adapter race suites for concurrent code;
- facts-v1 validation;
- semantic relation reports;
- byte-for-byte repeat-run comparison;
- positive and expected-negative relation gates;
- real-repository corpus cases across C, C#, GDScript, Java, Kotlin, Python, Ruby, Rust, TypeScript, JavaScript, and Svelte;
- C# smoke, incremental, determinism, and restored MSBuild project-graph checks;
- Java compiler-backed fixture coverage plus HikariCP and Apache Maven repository scans;
- Kotlin fixture, dependency, extension-call, runtime-semantics, and determinism checks;
- pinned Git, Codebase Memory, LevelDB, fmt, Catch2, and nlohmann/json judgments for the C/C++ shared adapter;
- call-site, possible-target-fanout, resolution-provenance, macro-expansion-depth, and direct argument-flow reporting for C-family corpus outputs;
- fixture and application smoke coverage for the C/C++ shared adapter;
- separate dated Go real-repository validation.

See:

- [Semantic acceptance gates](SEMANTIC_ACCEPTANCE.md)
- [Cross-adapter corpus validation](SEMANTIC_CORPUS_VALIDATION.md)
- [Go adapter validation](GO_ADAPTER_VALIDATION.md)
- [Evaluation harness](../evaluation/README.md)

## Explicit non-claims

Lexicon does not currently claim:

- perfect semantic precision or recall;
- complete runtime dispatch recovery for dynamic languages;
- runtime instrumentation;
- graph-query, reachability, impact-analysis, or ranking APIs;
- a complete dependency implementation graph for external packages whose source is not scanned;
- Svelte template, Astro template, procedural-macro expansion, Godot scene-tree inference, or framework-generated code semantics;
- that dated corpus counts or elapsed times remain unchanged after implementation updates.

High unresolved counts are expected where the language or framework permits runtime behavior that static evidence cannot prove. An unresolved record is preserved information, not automatically a defect.

## Documentation status

The root README is the entry point. `docs/README.md`, `adapters/README.md`, `spec/README.md`, `evaluation/README.md`, and `tools/README.md` index their owning documentation surfaces. Current behavior, normative contracts, adapter-specific semantics, and dated validation evidence are intentionally separated.

## Related docs

- [Application and operations](APPLICATION.md)
- [Lexicon architecture](ARCHITECTURE.md)
- [Semantic acceptance gates](SEMANTIC_ACCEPTANCE.md)
- [Semantic corpus validation](SEMANTIC_CORPUS_VALIDATION.md)

## Notes

Update this document in the same change whenever a capability, adapter, limit, or validation status materially changes.
