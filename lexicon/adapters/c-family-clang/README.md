# C-family Clang frontend

This directory owns Lexicon's private C/C++ compiler frontend process.

The helper is intentionally a process boundary. Clang/LLVM types do not cross into the Rust adapter; the helper emits a versioned JSON protocol and Rust remains responsible for Lexicon identities, fact policy, and storage.

Phase 2.2 implements only the permanent runtime/capabilities handshake. It does **not** own production C-family facts yet.

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

For protocol execution it reads exactly one JSON request line from stdin and writes exactly one JSON response line to stdout. Protocol v1 exposes the permanent `capabilities` operation used to prove the linked Clang runtime and compilation-database visibility.
