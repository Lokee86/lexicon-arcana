# C-family Clang frontend

This directory owns Lexicon's private C/C++ compiler frontend process.

The helper is a process boundary. Clang/LLVM types do not cross into the Rust adapter; it emits versioned compiler observations while Rust remains responsible for Lexicon identities, fact policy, canonicalization, and storage.

Phase 2.5 extends the staged path with compiler-bound arguments, indirect callee values, pointer bindings, reads/writes, and macro-expansion provenance on top of the Phase 2.4 relationship/call observations. It does **not** own production C-family facts yet.

## Requirements

Builds require a Clang/LLVM development installation that exports both `LLVMConfig.cmake` and `ClangConfig.cmake`. A compiler executable alone is not sufficient.

## Build

Use the repository build helper:

```text
python scripts/build_c_family_clang.py --output build/c-family-clang
```

If LLVM/Clang CMake packages are outside the normal CMake prefix path, pass `--llvm-dir` and `--clang-dir`.

## Runtime contract

The helper accepts:

- `--version`
- `--protocol-version <n>`
- `--helper-version <version>`

It reads one JSON request line from stdin and writes one JSON response line to stdout.

Protocol v1 exposes:

- `capabilities` — linked Clang version and compilation-database visibility;
- `structural` — translation-unit/build context plus source-language, declaration, include, macro, diagnostic, inheritance/override, call-target, overload, receiver-type, virtual-dispatch, compiler-bound argument/value-flow, pointer-binding, access, and macro-expansion observations.

The structural request receives the repository root and the Rust-discovered C-family source inventory. Compilation-database commands are used when available. Uncovered source files use synthesized Clang commands; headers are normally observed through source translation units, with only orphan headers receiving direct Clang analysis.

The protocol does not contain facts-v1 records or canonical Lexicon IDs.
