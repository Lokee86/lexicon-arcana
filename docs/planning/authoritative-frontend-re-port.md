# Authoritative Frontend Re-port

Parent index: [Planning](INDEX.md)

## Purpose

Replace Lexicon-owned or non-authoritative programming-language parsing and compiler-semantic reconstruction with each language's authoritative frontend, while preserving Lexicon's normalized fact model, canonical identities, deterministic publication, and graph-oriented semantic analysis.

The primary objective is **semantic authority**: Lexicon should not maintain another implementation of a language grammar or compiler semantics when the language implementation already provides them.

The secondary objective is **performance**: leverage the mature frontend's optimized parser, binder, type system, project model, caches, incremental machinery, and concurrency instead of reproducing those costs in Lexicon.

## Target architecture

```text
repository
    |
    v
language-native frontend
(parser / compiler / project model)
    |
    v
compact semantic observations
    |
    v
Rust LanguageAdapter
(normalization + Lexicon-specific analysis)
    |
    v
Analysis / facts-v1
    |
    v
Lexicon storage / Arcana
```

The language frontend owns authoritative syntax and compiler semantics.

Lexicon owns:

- canonical identities;
- facts-v1 materialization;
- source ownership;
- definite / possible / unresolved relationship policy;
- deterministic ordering;
- incremental replacement semantics;
- repository/dependency evidence outside compiler ownership;
- Lexicon-specific graph analysis not supplied directly by the frontend.

A helper protocol must carry semantic observations, not facts-v1 and not a universal AST.

## Performance principles

Performance is a first-class acceptance concern, not an after-the-fact optimization.

- Invoke language-native tooling at its natural semantic unit: project, package, crate, workspace, translation unit, or repository batch.
- Do not spawn one compiler/frontend process per source file.
- Measure end-to-end semantic analysis rather than raw parser throughput.
- Prefer compiler-native caches, project loading, parallelism, and incremental mechanisms before building Lexicon-specific substitutes.
- Optimize process startup and serialization only after profiling shows they materially obscure frontend gains.
- Do not introduce persistent helper daemons until ordinary bulk invocation has been measured and shown insufficient.

A richer authoritative frontend may perform more work than the old parser. Performance comparisons therefore require equivalent or improved semantic depth, not parser-only timings.

## Phase 0 — Freeze semantic and performance baselines

Before replacing another parser:

- retain current adapter fixtures as semantic oracles;
- record representative real-repository baselines for each affected language;
- measure cold wall time, warm wall time where meaningful, peak RSS, source volume, fact counts, unresolved counts, and output size;
- extend performance instrumentation beyond the current Go-heavy stage metrics.

Standardize observable stages such as:

```text
frontend.startup
frontend.project_load
frontend.parse
frontend.semantic_analysis
frontend.observation_emit
helper.ipc
lexicon.materialization
lexicon.semantic_extensions
lexicon.canonicalization
```

**Gate:** each language being migrated has a frozen semantic oracle and a meaningful performance baseline.

## Phase 1 — Establish the canonical frontend seam

Use the production Go adapter as the reference pattern:

```text
authoritative frontend -> semantic observations -> Rust adapter -> Analysis
```

Do not create a general plugin framework or universal compiler abstraction.

The observation seam should expose only reusable compiler evidence, for example:

- declarations and compiler identities;
- source spans;
- resolved symbol references;
- call targets or candidate sets;
- types and receiver types where useful;
- inheritance, implementation, and override evidence;
- modules/packages/imports;
- reads/writes where exposed;
- diagnostics and unresolved compiler evidence.

The Rust adapter remains responsible for translating observations into Lexicon semantics.

The Phase 1 ownership boundary is frozen in [ADR 0007](../decisions/0007-authoritative-frontends-emit-observations.md). It defines the Go reference observation vocabulary and explicitly keeps canonical identities, target ownership classification, call certainty, unresolved reasons, and facts-v1 materialization in Rust.

**Gate:** the seam is sufficient for multiple languages without adding a second facts contract or a universal AST schema.

## Phase 2 — C/C++ pilot: Tree-sitter to Clang

Replace Tree-sitter and compiler-like reconstruction with Clang frontend / LibTooling evidence.

Delete or substantially reduce custom ownership of:

- C/C++ parsing;
- preprocessing and macro interpretation where Clang provides truth;
- include semantics;
- overload/type resolution;
- receiver resolution;
- callable candidate reconstruction.

Use compilation-database/build context when available.

Retain Lexicon-specific fact normalization, repository ownership, uncertainty policy, graph-oriented value flow, and unresolved representation.

There is no Clang-to-Tree-sitter production fallback after cutover.

**Gate:** representative C and C++ corpora show equal-or-better semantic quality, measured wall/RSS results, deterministic facts, and only the Clang-backed production path remains.

## Phase 3 — Kotlin: bespoke lexer/parser to K2

Replace the Lexicon-owned Kotlin lexer, parser, navigation, declaration grammar, and compiler-like resolution with K2/compiler frontend evidence.

Preserve Lexicon-specific normalization and graph semantics.

Measure JVM/compiler startup separately from project-sized analysis so small-fixture overhead does not determine architecture.

**Gate:** K2 is the sole syntax/compiler authority for Kotlin.

## Phase 4 — GDScript: bespoke frontend to Godot

Replace Lexicon's GDScript lexer/parser and duplicated static-analysis semantics with Godot's GDScript parser/analyzer.

Keep Godot repository semantics that belong to Lexicon, such as resource-path, autoload, callback, and graph-oriented relationships where the compiler frontend does not already own them.

Isolate Godot internals behind a private helper if direct embedding would leak engine implementation details into Lexicon.

**Gate:** Lexicon owns no GDScript grammar.

## Phase 5 — Rust: syn reconstruction to rustc-backed analysis

Use a pinned Rust toolchain and private compiler helper.

Keep unstable `rustc_private` / compiler internals outside the Lexicon core interface.

Replace syntax, type, trait, call, and compiler-semantic reconstruction that rustc can answer; preserve graph-oriented analysis that rustc does not directly provide in Lexicon's required form.

**Gate:** one rustc-backed production path, with explicit toolchain incompatibility rather than silent semantic degradation.

## Phase 6 — Python: RustPython to CPython frontend

Replace RustPython as syntax authority with CPython parser/symbol-table evidence.

Retain Lexicon's repository-local binding, cross-file call inference, callback/value propagation, protocol handling, higher-order relationships, and error/outcome analysis where these are graph-analysis concerns rather than duplicated CPython semantics.

Batch source aggressively enough that process/runtime crossing does not dominate small-file parsing.

**Gate:** CPython is the syntax authority and RustPython parser dependency is removed from the production adapter.

## Phase 7 — Ruby: Tree-sitter to Prism

Replace Tree-sitter Ruby with Prism.

Retain higher-level conservative Ruby graph semantics where runtime dynamism prevents the frontend from proving a unique target.

**Gate:** Prism is the sole Ruby parser authority.

## Phase 8 — Java: complete the javac transition

Java already uses javac attribution but retains a custom parser/fallback semantic implementation.

Invert the relationship so javac is authoritative and Lexicon is the consumer. Delete compiler-semantic reconstruction that javac supplies.

Keep Java late in the sequence because the current development host lacks the required JDK environment.

**Gate:** javac owns Java syntax and compiler semantics; no independent Java parser remains merely as fallback.

## Phase 9 — Audit already-correct adapters

Do not rewrite correct adapters for symmetry.

Audit and retain:

- C# -> Roslyn / MSBuild;
- Go -> Go parser/types/packages/SSA/VTA helper;
- TypeScript/JavaScript -> TypeScript Compiler API.

Normalize boundary concepts only where they materially reduce duplication or improve performance. Do not force every language through identical helper internals.

## Phase 10 — Document LotusScript as an exception

Perform one focused search for a reusable authoritative LotusScript compiler/parser frontend.

If none is available under usable terms, document LotusScript as an explicit exception to the language-authoritative frontend invariant.

Its Lexicon-owned parser then remains deliberate ownership until an authoritative frontend becomes usable.

## Phase 11 — Performance deepening

After frontend cutovers, profile the family as a whole.

Optimize in this order:

1. compiler-native project/batch analysis;
2. compiler-native concurrency;
3. removal of duplicate parsing/resolution in Lexicon;
4. compact observation emission;
5. streaming or bounded serialization where payloads justify it;
6. reuse of frontend state within one scan;
7. persistent workers/daemons only when startup is proven material.

The default is to exploit mature compiler architecture before adding new Lexicon performance machinery.

## Migration strategy

Use a **staged hard cut, one language at a time**:

```text
freeze oracle + benchmark
        |
        v
build authoritative frontend path
        |
        v
calibrate semantic differences
        |
        v
measure wall time / RSS
        |
        v
cut production ownership
        |
        v
delete old parser/resolver
        |
        v
integration gate
```

For each language:

- keep one canonical production implementation;
- do not preserve the old parser as a fallback;
- do not add wrappers or compatibility aliases merely to keep an intermediate migration green;
- allow a scoped red migration window when necessary;
- verify the coherent migrated subgraph during that window;
- restore full repository build/test correctness at the language integration gate.

The real compatibility boundaries are facts-v1, canonical identities where semantics remain equivalent, snapshot/publication behaviour, deterministic ordering, and downstream consumers. Bugs or approximations in the retired parser are not compatibility requirements.

## Testing decisions

Completed language migrations are tested through:

- existing semantic fixture/oracle suites;
- facts-v1 validation;
- canonical identity/determinism checks;
- representative real-repository semantic calibration;
- frontend/toolchain diagnostics;
- end-to-end wall-time and peak-RSS benchmarks;
- incremental/full equivalence where the language supports narrowed analysis.

Semantic differences from the old implementation must be adjudicated. They are not automatically regressions when the authoritative frontend demonstrates that the previous adapter was wrong or incomplete.

## Completion gates

The re-port is complete when:

- no independently maintained programming-language grammar remains except documented exceptions;
- compiler semantics are not redundantly reconstructed where the authoritative frontend exposes them;
- each migrated language has one canonical production frontend path;
- old parser/resolver implementations and migration-only scaffolding are deleted;
- facts-v1 and publication contracts remain valid;
- deterministic output requirements remain satisfied;
- representative real-repository performance is measured for every migrated language;
- equivalent-or-improved semantic depth has no unexplained material wall-time or RSS regression;
- integration tests and whole-repository gates pass after each language cutover.

## Current sequencing

Planned migration order:

1. baseline and shared seam;
2. C/C++;
3. Kotlin;
4. GDScript;
5. Rust;
6. Python;
7. Ruby;
8. Java;
9. audit C#, Go, and TypeScript/JavaScript;
10. confirm/document LotusScript exception;
11. family-wide performance deepening.

C/C++ is the pilot because it combines substantial duplicated compiler machinery with the strongest opportunity to gain both semantic authority and performance from an established compiler frontend.
