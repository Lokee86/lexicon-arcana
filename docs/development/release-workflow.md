# Release workflow

Parent index: [Development Documentation](INDEX.md)

## Purpose

This document defines the build, test, packaging, installation, verification, and GitHub release workflow for the combined Grimoire distribution.

## Overview

The root workflow composes independently owned Grimoire, Lexicon, Arcana, Lodestone, and adapter artifacts while preserving their implementation and runtime boundaries.

The root workflow coordinates the independent Grimoire Go, Lexicon Go, Arcana Rust, Lodestone native, and Lexicon adapter build roots. It does not merge their implementation boundaries or generate source into component trees.

## Requirements

A complete source build expects:

- Python 3.12 or newer;
- Go 1.26.5;
- Rust 1.90 or newer;
- Node.js 22 for the TypeScript adapter;
- Lodestone checked out at `../lodestone`, or `LODESTONE_ROOT` set to another checkout, at commit `372abb7b9d5c9fb19eeaf92774849505e1dfbade`.

Use `py -3` instead of `python` when that is the configured Windows launcher.

## CPU bounds

Build, test, and release commands first run the Pitlord repository policy and verify the pinned Lodestone module/source identity. They default to one worker:

```bash
python scripts/workflow.py test
python scripts/workflow.py build --version 0.1.0-dev
python scripts/workflow.py release --version 1.2.3 --output dist
```

The bound applies to:

- Go package build concurrency;
- Go test parallelism;
- Cargo build jobs;
- Rust test threads.

This prevents the workflow from spawning a large package-test swarm. Increase concurrency only deliberately:

```bash
python scripts/workflow.py test --jobs 2
python scripts/workflow.py release --version 1.2.3 --jobs 2
```

`--jobs` must be at least 1. A high value can still overload the machine.

## Smoke checks

The deterministic packaging/install smoke suite does not compile every component:

```bash
python scripts/workflow.py smoke
python scripts/test_workflow.py
```

It validates archive layout, fixed metadata, checksums, version validation, component dependency-closure installation, bundled Lexicon adapter installation, skill installation, and concurrency defaults.

After producing a real build layout, run the release-consumer MCP smoke:

```bash
python scripts/test_installed_mcp.py --source build --version installed-smoke
```

This packages a combined release ZIP, extracts it, runs the ZIP's embedded installer into a clean temporary directory, verifies known C-family macro reasons and forward-compatible unknown unresolved-reason warning propagation through installed Lexicon, Arcana, and Grimoire, launches the installed `grimoire mcp`, prepares managed Lexicon/Arcana/Grimoire state, and verifies opaque search handles through `inspect` and `trace`. It does not rely on source-checkout provider paths.

## Build layout

```text
build/
  bin/
    grimoire(.exe)
    lexicon(.exe)
    arcana(.exe)
  native/
    lodestone_ffi.dll | liblodestone_ffi.so | liblodestone_ffi.dylib
  adapters/
    <Lexicon runtime adapters>
  skills/
    grimoire/
      SKILL.md
```

The Go CLIs receive the version through linker flags. Arcana receives the same value through `GRIMOIRE_RELEASE_VERSION`; a standalone Cargo build still reports its manifest version. After building, the workflow creates a minimal temporary Arcana snapshot and requires `arcana.query.v1` capability negotiation with the operations consumed by integration clients. A stale or protocol-incompatible Arcana executable therefore fails the build instead of producing a nominally version-valid bundle.

## Local installation

```bash
python scripts/workflow.py install --source build --bin-dir PATH
python scripts/workflow.py install --source build --bin-dir PATH --component grimoire
python scripts/workflow.py install --source build --bin-dir PATH --component lexicon --component arcana
```

Omitting `--component` installs the complete Grimoire runtime. Selecting `--component grimoire` also installs its required Lexicon and Arcana provider executables plus Lexicon runtime adapters. Selecting only `lexicon` or only `arcana` remains supported for independently usable component installations.

When Grimoire is selected, installation therefore copies:

- the Grimoire, Lexicon, and Arcana executables;
- the Lexicon runtime adapter tree;
- the Lodestone native library beside Grimoire;
- the canonical skill to `~/.agents/skills/grimoire/SKILL.md` and `~/.hermes/skills/grimoire/SKILL.md` by default.

When Lexicon is selected independently, its runtime adapters are also copied into the installed adapter tree.

Override skill roots or skip skill installation:

```bash
python scripts/workflow.py install --source build --bin-dir PATH --skills-dir /custom/skills
python scripts/workflow.py install --source build --bin-dir PATH --skip-skills
```

The installer does not modify `PATH` or agent-host configuration.

## Release packaging

```bash
python scripts/workflow.py release --version 1.2.3 --output dist
```

The command first runs the bounded complete test matrix, then builds and writes:

```text
dist/1.2.3/
  grimoire-1.2.3-<platform>-<arch>.zip
  lexicon-1.2.3-<platform>-<arch>.zip
  arcana-1.2.3-<platform>-<arch>.zip
  grimoire-bundle-1.2.3-<platform>-<arch>.zip
  release-manifest.json
  SHA256SUMS.txt
```

The Grimoire component archive contains its executable, required native library, and canonical skill. The combined bundle contains:

```text
bin/
native/
adapters/
skills/
install.py
VERSION
```

Archives use sorted entries and fixed timestamps for deterministic packaging. The local release command creates files only; it does not publish, tag, or push.

## GitHub release workflow

`.github/workflows/release.yml` builds Windows x86_64 and Linux x86_64 combined bundles on version tags or manual dispatch. It checks out Grimoire and Lodestone as sibling directories, pins Lodestone to the exact consumed commit, runs the same root release workflow, creates release-level checksums, and uploads the bundles to the GitHub release.

The workflow currently publishes combined bundles rather than every local component archive.

## Release verification

Before external publication:

1. Inspect `release-manifest.json`.
2. Verify `SHA256SUMS.txt`.
3. Exercise `grimoire version`, `grimoire lexicon check`, and `grimoire arcana check`.
4. Confirm the installed skill is byte-identical to `skills/grimoire/SKILL.md`.
5. Run a bounded discovery smoke test against a representative repository.
6. Confirm a fresh agent session discovers the installed skill and MCP tool.

See [Installation and agent setup](../reference/installation.md) for the consumer workflow.

## Code map

| Release concern | Primary implementation | Related tests or gates |
| --- | --- | --- |
| Root workflow orchestration | `scripts/workflow.py` | `scripts/test_workflow.py` |
| Local installation and layout | `scripts/install.py` | `scripts/test_installed_mcp.py` and smoke checks |
| GitHub release automation | `.github/workflows/release.yml` | workflow build/test/package jobs |
| Architecture and documentation gates | `tools/pitlord/`, `.github/workflows/documentation-standard.yml`, `scripts/check_docs.py` | Pitlord repository policy, source-identity tests, root and component documentation checks |
| Lexicon packaging | `lexicon/tools/package_release.py` | `lexicon/tools/test_package_release.py`, installer smoke tests |
| Arcana build artifact | `arcana/Cargo.toml`, `arcana/src/main.rs` | Cargo format/check/test gates |

Release automation composes independently owned components; it does not redefine their runtime contracts.

## Related docs

- [Installation and agent setup](../reference/installation.md)
- [Testing and benchmarks](testing-and-benchmarks.md)
- [Component architecture](../architecture/components.md)
- [Operations and trust boundaries](../architecture/operations-and-trust.md)
- [Architecture verification](architecture-verification.md)
- [Lexicon release packaging](../../lexicon/docs/RELEASE_PACKAGING.md)

## Notes

Release composition does not make the root workflow the owner of component-internal formats or behavior.
