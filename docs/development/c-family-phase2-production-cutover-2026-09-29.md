# C-family Phase 2.6 production Clang cutover

Parent index: [Development Documentation](INDEX.md)

**Date:** 2026-09-29

**Branch:** `refactor/c-family-clang`

**Starting checkpoint:** `a411b4a` — `Rebase C-family value flow on Clang observations`

## Purpose

Phase 2.6 makes Clang/LibTooling the sole production C/C++ syntax and compiler-semantic authority.

The production path is now:

```text
Rust source discovery
    |
    v
private Clang/LibTooling frontend
    |
    | compiler observations
    v
Rust C-family materialization
    |
    | canonical identities + graph/fact policy
    v
facts-v1
```

There is no Tree-sitter C/C++ production path and no Clang-to-Tree-sitter fallback.

## Production ownership

`AdapterHost` constructs `CFamilyAdapter` with the configured adapter root. `CFamilyAdapter::analyze()` canonicalizes the repository, discovers the bounded C-family source inventory in Rust, invokes `ClangFrontend::structural()`, materializes compiler observations, and applies Rust-owned Lexicon identity/graph/fact policy.

The adapter fails closed when the packaged Clang helper cannot be resolved or executed.

## Retired implementation

The hard cut deletes the former C/C++ Tree-sitter and compiler-reconstruction modules rather than retaining them as a fallback. This includes syntax parsing/fallback, declaration/call/inheritance extraction, custom call/receiver/inheritance resolution, syntax-derived dataflow, custom pointer/indirect-call reconstruction, Rust macro call reconstruction/substitution, and migration-only legacy model shapes/tests.

The `tree-sitter-c` and `tree-sitter-cpp` Rust dependencies are removed.

## Preserved Rust responsibilities

Rust continues to own repository discovery, canonical Lexicon IDs/path ownership, local include/file visibility, compiler-reference mapping, definite/possible/unresolved policy, fixed-point callback propagation over compiler-bound identities, `passes-to`/`reads`/`writes` materialization, deterministic deduplication, canonicalization, validation, and publication.

These are Lexicon semantics over compiler evidence, not an independent C/C++ parser or binder.

## Calibration

The Phase 2.1 oracle remains historical adjudication evidence, not a byte-for-byte target after the authoritative frontend cut.

An old structural fixture expected Rust to infer a macro alias target by parsing macro replacement text. That expectation was intentionally removed: preprocessing/macro expansion belongs to Clang, and Rust does not reconstruct macro call semantics after cutover.

Frozen canonical callable identity vectors remain protected where compiler observations map to equivalent Lexicon entities.

## Verification

The cutover is protected by staged Clang structural/semantic/value-flow materialization tests; a production-route test that drives `CFamilyAdapter::analyze()` through the private frontend; complete Rust tests; Doctor/runtime-helper discovery tests; documentation and packaging workflow checks; and searches proving C/C++ Tree-sitter dependencies and retired modules are gone.

Verification on the current Windows host:

- focused C-family Clang/cutover suite: 16/16 passed;
- complete Lexicon Rust test suite: passed, including integration and doc tests;
- `lexicon-cli` Cargo check: passed after regenerating its lockfile;
- workflow packaging/install smoke tests: 9/9 passed;
- documentation validation: 203 Markdown files passed;
- active Rust manifests, lockfiles, and C-family source contain no `tree-sitter-c` / `tree-sitter-cpp` dependency or import;
- root Lexicon Go packages passed before the root workflow reached the unrelated Java adapter;
- the root workflow then stopped because this host has no Java runtime;
- the multilang performance runner reached the real C-family helper Release build and stopped because `LLVMConfig.cmake` is unavailable.

Native helper/runtime and C-family performance acceptance therefore remain environment-blocked exactly at the pre-declared LLVM development-package boundary. A compiler executable alone is insufficient, and Phase 2.6 does not install host LLVM packages.

## Result

After Phase 2.6 there is one canonical C-family production architecture:

```text
Clang compiler truth -> private observations -> Rust Lexicon policy -> facts-v1
```

No compatibility shim, dual analyzer, or fallback path survives the migration.