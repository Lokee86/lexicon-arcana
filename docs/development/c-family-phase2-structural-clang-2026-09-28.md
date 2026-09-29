# C-family Phase 2.3 structural Clang observations

Parent index: [Development Documentation](INDEX.md)

**Date:** 2026-09-28

**Branch:** `refactor/c-family-clang`

**Starting checkpoint:** `df52d789157f66b8a000bb039ce335a88443d079` — `Establish C-family Clang runtime boundary`

**Helper version:** `0.2.0`

## Purpose

Phase 2.3 establishes the staged structural-analysis path that will replace C-family Tree-sitter ownership at the Phase 2.6 production cutover.

This phase deliberately does **not** change `CFamilyAdapter::analyze()`. Production facts still come from the existing Rust/Tree-sitter implementation so there is one production path while later compiler relationships and Lexicon-specific dataflow are migrated.

The staged path is:

```text
repository inventory
    -> one ClangTool repository batch
       -> compilation-database commands when present
       -> conservative synthesized commands otherwise
       -> headers primarily through their real translation units
       -> orphan headers through a synthesized Clang context
    -> structural semantic observations
    -> Rust structural materialization
    -> existing RepositoryModel / facts policy
```

## Clang-owned structural evidence

The private helper now defines a `structural` protocol operation and emits:

- translation units and the actual compiler command used;
- C versus C++ frontend context;
- declarations and helper-local compiler identities;
- namespaces, records/enums, typedef/type aliases;
- functions, methods, constructors, parameters, fields, variables, and constants;
- source-written callable declarators;
- source spans;
- compiler type spelling where structurally useful;
- include directives plus Clang's resolved repository-relative include path;
- macro definitions, parameters, replacement tokens, and conditional context;
- compiler diagnostics.

Inheritance and call binding are intentionally deferred to Phase 2.4. Pointer/callback/dataflow rebasing is intentionally deferred to Phase 2.5.

## Build-context policy

When Clang can load a compilation database, covered source files use those real commands.

For sources without a compile command, the helper synthesizes one Clang invocation using the source extension:

- `.c` -> C11;
- other explicit source extensions -> C++17.

Headers are not eagerly compiled as independent translation units. Source roots run first; headers observed through those compilations inherit the real frontend language/TU context. Only requested headers that were not reached through a source translation unit are analyzed directly. Explicit C++ header extensions use C++; ambiguous orphan `.h`/`.inc` files are tried under both synthesized C and C++ Clang contexts, after which Rust retains the existing content-language policy for the single normalized file fact. If a header is reached from both real C and C++ translation units, the existing C-first normalization policy is preserved.

The helper is invoked once for the requested repository batch rather than spawning one helper process per file.

## Identity boundary

Clang identities are correlation keys only. They are never persisted as Lexicon node IDs.

Rust recomputes C-family identities from the existing Lexicon-owned canonical form:

```text
repository-relative owner
+ Lexicon kind
+ qualified name
+ source-written callable signature where required
```

Compiler-ID translation is file-local. The same Clang USR observed in multiple source owners therefore cannot accidentally collapse path-owned Lexicon nodes.

Callable signatures deliberately preserve the source-written declarator, including parameter names and qualifiers where present. This keeps equivalent existing identities stable; for example the frozen `add(int left, int right)` identity does not silently become a new `add(int,int)` identity merely because Clang can normalize the type.

The staged tests also retain the previous adapter's lexical classification for out-of-class member definitions until later semantic relationship work: compiler authority improves evidence without forcing an unrelated canonical-ID migration in this subphase.

## Include ownership

Clang now supplies both:

- the include target as written;
- the resolved repository-relative file path when the compiler resolves it.

The shared Rust `FileIndex` prefers that resolved path when it is present. The old heuristic include resolver remains reachable only for the still-live Tree-sitter production model, whose observations carry an empty resolved path.

That makes the staged Clang path authoritative for include resolution without changing current production output before cutover.

## Rust materialization

`clang_materialization.rs` translates structural observations into the existing `RepositoryModel`.

It owns:

- repository-relative path validation;
- canonical Lexicon IDs;
- container/parent ID translation;
- structural attributes;
- file/module/declaration facts inputs;
- include model construction;
- deterministic ordering;
- compiler-error projection into the existing parse-error/unresolved surface.

The model now records its parser/frontend source explicitly so the staged path can report `clang` while the current production path continues reporting `tree-sitter`.

## Header and diagnostic behavior

One file may be observed from multiple translation units. The protocol records all observed language/TU contexts; Rust retains the pre-cutover C-family file-language policy when one normalized file fact must be materialized.

Compiler errors associated with repository files mark that file as structurally erroneous. Helper-global tool failures remain explicit diagnostics rather than triggering another parser.

## Verification and calibration

Phase 2.3 adds focused tests for:

- versioned structural protocol execution;
- structural observation -> Lexicon materialization;
- file-local compiler identity translation;
- Clang-resolved include ownership;
- compiler diagnostic propagation;
- noncanonical/escaping path rejection;
- frozen callable identity vectors.

The Phase 2.1 canonical facts oracle remains a **production non-regression gate** during this staged phase. It is not an assertion that future Clang structural output must preserve known Tree-sitter mistakes.

## Native helper build limitation

The current Windows host and its Ubuntu WSL environment do not have LLVM/Clang development CMake packages installed. Consequently the LibTooling helper still cannot be linked locally.

The C++ implementation has been checked against the current Clang API documentation and the Rust/process boundary is executable through scripted helper tests, but a real native helper compile remains an environment-dependent verification item until an LLVM/Clang development SDK is available.

No host or WSL packages were installed during this phase.

## Phase 2.3 gate

Phase 2.3 is complete when:

- the `structural` helper protocol is defined;
- repository/TU build context is represented explicitly;
- structural declarations, includes, macros, language context, and diagnostics are emitted by the Clang path;
- Rust materializes those observations using Lexicon-owned identities and fact policy;
- resolved includes prefer compiler evidence;
- frozen equivalent callable identity vectors remain stable;
- current production C-family facts remain byte-identical to the Phase 2.1 oracle;
- C-family tests, documentation governance, and the registered multilang regression gate pass;
- no production fallback or dual facts execution has been added.

## Next

Phase 2.4 moves compiler-owned semantic relationships onto this path: inheritance, overrides, direct/member calls, overload selection, receiver/type evidence, and compiler candidate sets.
