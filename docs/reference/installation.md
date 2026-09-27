# Installation

Parent index: [Reference](INDEX.md)

## Purpose

Define installation and source-build behavior for the active Lexicon + Arcana product family.

## Overview

For normal Lexicon use, use the last optimized Go implementation pinned at `758af9daf6e71fc0a7ebb837875efe366f6403fd`. The Rust Lexicon port is still under active migration and is not yet the recommended operator runtime because its optimization work is incomplete.

Arcana remains the current Rust graph component. Grimoire itself is retired and is not required at runtime.

## Prerequisites

The recommended Go Lexicon source build requires Go 1.26.5 plus the language runtimes needed by its adapters. Arcana requires Rust 1.90 or newer. Node.js 22 is required for the TypeScript adapter, with the other adapter-specific runtimes documented by Lexicon.

The shared root packaging workflow currently produces the Rust Lexicon migration binary; do not treat that bundle as the preferred Lexicon runtime until the optimization gap is closed.

## Recommended Lexicon installation

Check out the pinned optimized Go revision:

```text
758af9daf6e71fc0a7ebb837875efe366f6403fd
```

From that checkout, build Lexicon from `lexicon/`:

```bash
go build -o ../bin/lexicon ./cmd/lexicon
```

Build Arcana separately from the repository root:

```bash
cargo build --release --locked --manifest-path arcana/Cargo.toml
```

Place the resulting `lexicon` and `arcana` executables on `PATH` or invoke them by explicit path. Keep the Lexicon adapter tree available through the checkout, an adjacent packaged layout, `--adapters`, or `LEXICON_ADAPTERS`.

## Rust migration bundle

The current shared `scripts/workflow.py` build/release path packages the Rust Lexicon migration binary together with Arcana. It remains useful for migration testing and integration work, but it is not the recommended Lexicon runtime until its optimization work is complete.

## Component selection

Arcana may be built and installed independently of Lexicon. For Lexicon, prefer the pinned Go runtime above rather than the current Rust bundle.

The shared migration installer still supports component selection for development/testing:

```bash
python install.py --bin-dir /path/on/your/PATH --component lexicon
python install.py --bin-dir /path/on/your/PATH --component arcana
```

A `lexicon` installed this way is the Rust migration binary, not the current operator recommendation. `grimoire` is not a valid component and no Grimoire runtime is installed.

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

Recommended Lexicon runtime:

```bash
git checkout 758af9daf6e71fc0a7ebb837875efe366f6403fd
cd lexicon
go build -o ../bin/lexicon ./cmd/lexicon
```

Arcana:

```bash
cd ..
cargo build --release --locked --manifest-path arcana/Cargo.toml
cargo test --all-targets --locked --manifest-path arcana/Cargo.toml
```

For Rust Lexicon migration development, use the current branch and run:

```bash
python scripts/workflow.py build --version 0.1.0-dev --component lexicon
cargo test --all-targets --locked --manifest-path lexicon/Cargo.toml
cargo test --all-targets --locked --manifest-path lexicon-cli/Cargo.toml
```

The migration workflow defaults to one worker across Go and Cargo. Use `--jobs N` only when additional concurrency is intentional.

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
