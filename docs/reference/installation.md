# Installation

Parent index: [Reference](INDEX.md)

## Purpose

Define installation and source-build behavior for the active Lexicon + Arcana product family.

## Overview

The supported shared distribution installs Lexicon and Arcana directly. It contains no Grimoire runtime, MCP server, Grimoire skill, or Lodestone native library.

## Prerequisites

Release installation requires Python 3.12 or newer, a writable binary directory, and permission to add that directory to `PATH`. Source builds additionally require Go 1.26.5, Rust 1.90 or newer, and Node.js 22 for the TypeScript adapter.

## Release installation

Download and extract the combined bundle for the target platform:

```text
lexicon-arcana-bundle-<version>-<platform>-<arch>.zip
```

The bundle contains:

```text
bin/                    Lexicon and Arcana executables
adapters/               Lexicon runtime adapters
skills/lexicon-arcana/  Production agent skill
install.py              Standalone installer
VERSION                  Bundle version
```

Run:

```bash
python install.py --bin-dir /path/on/your/PATH
```

On Windows, `py -3` may be the configured Python launcher.

The installer does not modify `PATH`.

## Component selection

Omitting `--component` installs Lexicon + Arcana. Either product can also be installed independently:

```bash
python install.py --bin-dir /path/on/your/PATH --component lexicon
python install.py --bin-dir /path/on/your/PATH --component arcana
```

Selecting Lexicon also installs its runtime adapter tree. When both Lexicon and Arcana are selected, the installer also installs `lexicon-arcana/SKILL.md` to `~/.agents/skills` and `~/.hermes/skills` by default. Repeat `--skills-dir PATH` to choose explicit agent skill roots, or use `--skip-skills` for binaries/adapters only. A single-component installation does not install the combined skill. `grimoire` is not a valid component and no Grimoire binary, MCP server, native Lodestone library, or Grimoire skill is installed.

## Verify the installation

Run:

```bash
lexicon version
arcana --version
```

## Expected result

A successful installation reports the requested Lexicon and Arcana versions, exposes Lexicon runtime adapters when Lexicon is installed, and can produce a Lexicon snapshot that Arcana accepts for synchronization and bounded graph queries.

Then verify the deterministic repository-analysis path against a repository:

```bash
lexicon init --repo /path/to/repository
arcana sync \
  --lexicon /path/to/repository/.lexicon \
  --state /path/to/repository/.arcana \
  --register
```

Later repository changes use the normal Lexicon lifecycle:

```bash
lexicon scan --repo /path/to/repository
```

Arcana can also be synchronized explicitly when needed.

## Source build

Requirements:

- Python 3.12 or newer;
- Go 1.26.5;
- Rust 1.90 or newer;
- Node.js 22 for the TypeScript adapter.

From the repository root:

```bash
python scripts/workflow.py build --version 0.1.0-dev
python scripts/workflow.py install --source build --bin-dir /path/on/your/PATH
```

Run verification with:

```bash
python scripts/workflow.py smoke
python scripts/workflow.py test
```

The workflow defaults to one worker across Go and Cargo. Use `--jobs N` only when additional concurrency is intentional.

## Direct component builds

Lexicon:

```bash
cd lexicon
go build -o ../bin/lexicon ./cmd/lexicon
go test ./...
```

Arcana:

```bash
cargo build --release --locked --manifest-path arcana/Cargo.toml
cargo test --all-targets --locked --manifest-path arcana/Cargo.toml
```

## Agent integration

The retired Grimoire MCP/skill surface is not installed. Agents and higher-level consumers should combine:

- direct source/Git/file tools for exact implementation evidence;
- Lexicon semantic facts/snapshots when language-semantic ownership is useful;
- Arcana protocol queries for bounded graph questions.

The production skill is `skills/lexicon-arcana/SKILL.md`. It discovers normal repository-owned `.lexicon/` and `.arcana/` state through the public component commands, uses Lexicon for semantic facts and Arcana for bounded graph questions, and keeps direct source inspection authoritative. The separate skill under `evaluation/skills/` remains a frozen benchmark condition and may use benchmark-only environment variables; it is not installed.

## Optional Arcana semantic vectors

Arcana can explicitly build its optional semantic graph index against a compatible OpenAI-style embedding endpoint. The embedding runtime is external to Arcana; the active release does not include Lodestone or the retired Grimoire model runtime.

See [Arcana vector index](../../arcana/docs/vector-index.md).

## Failure and recovery

### Command not found

Confirm the selected binary directory is on the current shell's `PATH`.

### Lexicon cannot find an adapter

Install Lexicon from the combined bundle or ensure the adapter tree remains beside the installed Lexicon command in the expected layout.

### Arcana synchronization fails

Check that the Lexicon repository state is a current, valid published snapshot. Arcana records and validates the exact Lexicon snapshot it consumes.

### Agent expects `grimoire` or `grimoire_discover`

That integration is retired. Remove the old MCP/skill configuration and use direct Lexicon/Arcana plus normal repository tools. Historical Grimoire benchmark configurations remain available only as historical evidence.

## Related docs

- [Release workflow](../development/release-workflow.md)
- [Lexicon documentation](../../lexicon/docs/README.md)
- [Arcana documentation](../../arcana/docs/README.md)
- [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md)

## Notes

The combined bundle is an installation convenience only. Lexicon and Arcana remain independently installable products and no umbrella Grimoire process is installed.
