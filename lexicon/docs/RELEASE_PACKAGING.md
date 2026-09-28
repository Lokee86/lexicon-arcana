# Lexicon release packaging

Parent index: [Lexicon Documentation](README.md)

## Purpose

Define the current Lexicon build, bundle, installation, runtime-asset, and release-verification path while the parity-first Rust migration is in progress.

## Overview

The shared root workflow currently builds the Rust `lexicon-cli` migration executable and packages the runtime assets still required by supported language surfaces. That Rust build is not yet the recommended operator runtime because its optimization work is incomplete.

For normal Lexicon use, direct users to the last optimized Go implementation pinned at `758af9daf6e71fc0a7ebb837875efe366f6403fd`. The root Rust release workflow remains useful for migration testing and combined Lexicon + Arcana integration work.

## Build

From the shared repository root, build Lexicon only:

```text
python scripts/workflow.py build --version <version> --component lexicon
```

Build Lexicon + Arcana:

```text
python scripts/workflow.py build --version <version>
```

The disposable build layout contains:

```text
build/
  bin/
    lexicon
    arcana       # combined build
  adapters/      # Lexicon runtime adapter assets
  skills/        # combined build
  LICENSE.md
  LICENSING.md
  THIRD_PARTY_NOTICES.md
```

On Windows, executables use the `.exe` suffix.

## Release archives

Create tested release archives and SHA-256 checksums with:

```text
python scripts/workflow.py release --version <version>
```

The release directory contains independent Lexicon and Arcana archives plus a combined `lexicon-arcana-bundle-<version>-<platform>-<arch>.zip`.

The combined bundle contains `bin/`, Lexicon `adapters/`, the production Lexicon + Arcana agent skill, `install.py`, legal files, and `VERSION`.

## Installation

From an extracted combined bundle:

```text
python install.py --bin-dir /path/on/your/PATH
```

Install only Lexicon with:

```text
python install.py --bin-dir /path/on/your/PATH --component lexicon
```

The installer does not modify `PATH`. See the shared [installation guide](../../docs/reference/installation.md) for component selection and agent-skill installation.

## Build requirements

A complete release currently requires the toolchains used by both the Rust replacement and remaining packaged/oracle runtimes:

- Python 3.12 or newer;
- Rust 1.90 or newer;
- Go 1.26.5;
- Node.js 22;
- JDK 21 or newer;
- a compatible .NET SDK;
- other language runtimes required by supported transitional adapter paths.

These requirements will shrink as remaining language adapters complete migration into the Rust library.

## Runtime recommendation during migration

The optimized Go Lexicon at `758af9daf6e71fc0a7ebb837875efe366f6403fd` is the current operator recommendation.

The root release workflow's `lexicon` executable is produced by `lexicon-cli/Cargo.toml` and delegates to the Rust `lexicon` library. Treat that binary as a migration/development artifact until the outstanding optimization work is complete.

Some packaged adapter assets remain external during migration. The retired `lexicon-go` standalone facts adapter is not built, and its oracle source tree is excluded from runtime archives. Native Go analysis belongs to the Rust `GoAdapter`, and its private `lexicon-go-semantic[.exe]` helper plus `VERSION` are packaged under `adapters/go-semantic/`, version-verified, archived, and installed with Lexicon. There is no legacy facts-v1 runtime fallback. The helper itself is prebuilt, but typed Go semantics use `go/packages`, so an installed `go` executable remains a runtime prerequisite. Exact migration status is maintained in [Rust migration](RUST_MIGRATION.md).

## Verification

Before publishing a release:

1. run `python scripts/workflow.py test`;
2. build with the intended version;
3. verify `lexicon version` from the build output;
4. initialize a temporary mixed-language repository;
5. run `status`, `doctor`, `scan`, and representative semantic lookups;
6. for a combined build, synchronize Arcana and verify its protocol capabilities;
7. run `python scripts/workflow.py smoke`;
8. create release archives and verify `SHA256SUMS.txt`;
9. confirm generated build state, caches, tests, and evaluation output are not included unintentionally.

## Generated state

Release binaries and bundle contents are build artifacts. Repository analysis state created by an installed Lexicon remains under the analyzed repository's `.lexicon/` directory and should normally be ignored by that source repository.

The release process must not package repository-local `.lexicon/` or `.arcana/` state.

## Code map

| Packaging concern | Current implementation | Verification |
| --- | --- | --- |
| Root build/test/install/release workflow | `../scripts/workflow.py` | `../scripts/test_workflow.py` |
| Rust Lexicon executable | `../lexicon-cli/Cargo.toml`, `../lexicon-cli/src/` | Lexicon CLI tests |
| Lexicon library | `Cargo.toml`, `src/` | Lexicon library tests |
| Runtime adapter staging | `../scripts/workflow.py::package_lexicon_adapters` | workflow smoke/test path |
| Combined installer | `../scripts/install.py` | workflow smoke path |
| Release archive/checksum assembly | `../scripts/workflow.py::package_artifacts` | workflow tests |

## Related docs

- [Operator how-to](HOWTO.md)
- [Development and verification](DEVELOPMENT.md)
- [Rust migration](RUST_MIGRATION.md)
- [Shared installation guide](../../docs/reference/installation.md)
- [Root release workflow](../../docs/development/release-workflow.md)

## Notes

The root workflow is the release authority during migration. Component-local legacy build scripts or adapter executables may remain for parity and runtime support without defining the product-level build boundary.
