# Release workflow

Parent index: [Development Documentation](INDEX.md)

## Purpose

Define the build, test, packaging, installation, verification, and GitHub release workflow for Lexicon + Arcana.

## Overview

The root workflow composes two independently owned products:

- Lexicon: Go application plus runtime language adapters;
- Arcana: Rust application and `arcana.query.v1` protocol.

Grimoire, its MCP/discovery runtime, its skill, and Lodestone are not active release inputs. Historical Grimoire source and benchmark evidence may remain in the repository during retirement, but the release workflow does not build or package them.

## Requirements

A complete source build expects:

- Python 3.12 or newer;
- Go 1.26.5;
- Rust 1.90 or newer;
- Node.js 22 for the TypeScript adapter.

Use `py -3` instead of `python` when that is the configured Windows launcher.

## CPU bounds

The root workflow defaults to one worker across Go and Cargo:

```bash
python scripts/workflow.py test
python scripts/workflow.py build --version 0.1.0-dev
python scripts/workflow.py release --version 1.2.3 --output dist
```

Increase concurrency only deliberately:

```bash
python scripts/workflow.py test --jobs 2
python scripts/workflow.py release --version 1.2.3 --jobs 2
```

`build` also accepts repeatable component selectors:

```bash
python scripts/workflow.py build --version dev --component lexicon
python scripts/workflow.py build --version dev --component arcana
python scripts/workflow.py build --version dev --component lexicon --component arcana
```

Only `lexicon` and `arcana` are valid active components.

## Tests

`python scripts/workflow.py test` runs:

1. the repository Pitlord policy;
2. documentation validation;
3. Lexicon Go tests;
4. Java and Kotlin adapter Go tests;
5. the C# adapter Python test;
6. Arcana Rust tests.

The retired root Grimoire Go module is no longer part of the active test matrix.

The deterministic packaging/install smoke suite is:

```bash
python scripts/workflow.py smoke
python scripts/test_workflow.py
```

It verifies component selection, archive layout, checksums, adapter installation, version validation, protocol capability checks, and concurrency bounds.

## Build layout

A complete build produces:

```text
build/
  bin/
    lexicon(.exe)
    arcana(.exe)
  adapters/
    <Lexicon runtime adapters>
  skills/
    lexicon-arcana/
      SKILL.md
```

Lexicon receives the requested release version through Go linker flags. Arcana receives it through `ARCANA_RELEASE_VERSION`; standalone Cargo builds fall back to the package version in `Cargo.toml`.

After Arcana is built, the workflow creates a minimal temporary snapshot and requires compatible `arcana.query.v1` capability negotiation. A protocol-incompatible Arcana binary therefore fails the build.

## Local installation

Install both products:

```bash
python scripts/workflow.py install --source build --bin-dir /path/on/your/PATH
```

Install a single product:

```bash
python scripts/workflow.py install --source build --bin-dir /path/on/your/PATH --component lexicon
python scripts/workflow.py install --source build --bin-dir /path/on/your/PATH --component arcana
```

Selecting Lexicon also installs its runtime adapter tree. When Lexicon and Arcana are installed together, the installer writes the production `lexicon-arcana` skill to `~/.agents/skills` and `~/.hermes/skills` by default. Use repeatable `--skills-dir` options to choose other roots or `--skip-skills` to omit it. Installing only one component does not install the combined skill. The installer does not modify `PATH` and never installs a Grimoire skill or MCP server.

## Release packaging

```bash
python scripts/workflow.py release --version 1.2.3 --output dist
```

The release directory contains:

```text
dist/1.2.3/
  lexicon-1.2.3-<platform>-<arch>.zip
  arcana-1.2.3-<platform>-<arch>.zip
  lexicon-arcana-bundle-1.2.3-<platform>-<arch>.zip
  release-manifest.json
  SHA256SUMS.txt
```

The combined bundle contains:

```text
bin/
adapters/
skills/
  lexicon-arcana/
    SKILL.md
install.py
VERSION
LICENSE.md
LICENSING.md
THIRD_PARTY_NOTICES.md
```

Archives use sorted entries and fixed timestamps for deterministic packaging. Local release commands create files only; they do not publish, tag, or push.

## GitHub release workflow

`.github/workflows/release.yml` builds Windows x86_64 and Linux x86_64 combined bundles on version tags or manual dispatch. It checks out only this repository, runs the same root release workflow, and uploads `lexicon-arcana-bundle-*` artifacts.

GitHub releases are titled `Lexicon + Arcana <version>`.

## Release verification

Before external publication:

1. inspect `release-manifest.json`;
2. verify `SHA256SUMS.txt`;
3. exercise `lexicon version`;
4. exercise `arcana --version`;
5. run a representative Lexicon scan;
6. synchronize Arcana from the resulting Lexicon snapshot and run a bounded protocol query;
7. confirm `skills/lexicon-arcana/SKILL.md` is present in the combined bundle and is installed byte-identically to a selected skill root.

## Code map

| Release concern | Primary implementation | Related tests or gates |
| --- | --- | --- |
| Root workflow | `scripts/workflow.py` | `scripts/test_workflow.py` |
| Bundle installer | `scripts/install.py` | workflow smoke checks |
| GitHub release | `.github/workflows/release.yml` | release job |
| Repository policy/docs | `tools/pitlord/`, `scripts/check_docs.py` | root test workflow |
| Lexicon packaging | `lexicon/tools/package_release.py` | Lexicon packaging tests |
| Arcana artifact/protocol | `arcana/Cargo.toml`, `arcana/build.rs`, `arcana/src/` | Cargo tests and protocol capability check |

## Related docs

- [Installation](../reference/installation.md)
- [Component architecture](../architecture/components.md)
- [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md)
- [Lexicon release packaging](../../lexicon/docs/RELEASE_PACKAGING.md)

## Notes

The shared workflow composes independently owned artifacts. It does not restore Grimoire runtime behavior or redefine Lexicon/Arcana internal contracts.
