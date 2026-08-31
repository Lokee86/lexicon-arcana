# Installation and agent setup

Parent index: [Reference](INDEX.md)

## Purpose

This guide defines release installation, source builds, command verification, agent-skill setup, MCP configuration, first-run preparation, and recovery from common setup failures.

## Overview

The combined bundle installs Grimoire, Lexicon, Arcana, Lexicon adapters, the Lodestone native library, and the canonical Grimoire skill while preserving independent component executables and state.

## Prerequisites

Release installation requires Python 3.12 or newer to run the installer, a writable destination for the executables, and permission to add that destination to the user's `PATH`. Source builds additionally require the toolchain listed under [Build from source](#build-from-source).

This page covers release installation, source builds, command verification, agent skill installation, MCP configuration, first-run state preparation, and common setup failures.

## Recommended release installation

Download and extract the combined release bundle for the target platform:

```text
grimoire-bundle-<version>-<platform>-<arch>.zip
```

The bundle contains:

```text
bin/                    Grimoire, Lexicon, and Arcana executables
native/                 Lodestone native library
adapters/               Lexicon runtime adapters
skills/grimoire/        Canonical Grimoire agent skill
install.py              Standalone installer
VERSION                  Bundle version
```

Run the installer from the extracted bundle:

```bash
python install.py --bin-dir /path/on/your/PATH
```

On Windows, `py -3` may be the configured launcher:

```powershell
py -3 install.py --bin-dir "$HOME\bin"
```

The installer does not modify `PATH`. Add the selected binary directory through the operating system or shell configuration.

### Component selection

Omitting `--component` installs the complete Grimoire runtime. Selecting Grimoire explicitly installs the same required provider closure:

```bash
python install.py --bin-dir /path/on/your/PATH --component grimoire
```

That installs Grimoire together with Lexicon, Arcana, Lexicon runtime adapters, the required Lodestone native library, and the Grimoire agent skill. Lexicon and Arcana remain independently installable when only a component engine is wanted:

```bash
python install.py --bin-dir /path/on/your/PATH --component lexicon
python install.py --bin-dir /path/on/your/PATH --component arcana
```

Keeping the three Grimoire-runtime executables in one installation directory gives internal and downstream consumers one coherent provider family. Repository preparation also records Lexicon's adapter root, which provides a stable installation anchor for consumers that need to locate the matching Arcana executable without depending solely on `PATH`.

### Skill installation

By default, a Grimoire installation writes the canonical skill to both supported user roots:

```text
~/.agents/skills/grimoire/SKILL.md
~/.hermes/skills/grimoire/SKILL.md
```

Override the roots by repeating `--skills-dir`:

```bash
python install.py --bin-dir /path/on/your/PATH \
  --skills-dir /first/agent/skills \
  --skills-dir /second/agent/skills
```

Install only binaries and runtime files with:

```bash
python install.py --bin-dir /path/on/your/PATH --skip-skills
```

Start a new agent session after installing or updating a skill so the host can rediscover it.

## Verify the installation

Run:

```bash
grimoire version
grimoire lexicon check
grimoire arcana check
grimoire help
```

Expected behavior:

- `grimoire version` reports the installed release or development version;
- `lexicon check` reports the resolved Lexicon command and version;
- `arcana check` reports the resolved Arcana command and version;
- `grimoire help` lists direct discovery, MCP, state, vector, model, investigation, and engine namespace commands.

When provider discovery fails, verify that all bundle executables were installed together or use the explicit environment overrides documented in [CLI environment variables](cli.md#environment-variables).

## Expected result

A successful installation reports matching Grimoire, Lexicon, and Arcana versions, lists the unified command surface, prepares a representative repository without contract errors, and allows a narrow search or MCP request to return inspectable evidence handles.

## Prepare a repository

Grimoire stores repository-owned prepared state and refreshes it on demand. Check and prepare the current repository with:

```bash
grimoire status --root . --refresh
```

Then run a narrow code-first search:

```bash
grimoire search \
  --root . \
  --query "Where is session creation handled?" \
  --breadth narrow \
  --code-only \
  --session session-flow
```

Narrow search defaults to four handle-only results. Inspect selected handles to retrieve exact source.

The first preparation can cost more than later queries because Grimoire may need to align source, Lexicon, Arcana, and documentation state. Normal discovery uses `refresh-if-needed`; do not use `force-refresh` as a routine first step.

## Configure an agent host

Grimoire serves one stdio MCP tool:

```bash
grimoire mcp --root /absolute/path/to/repository
```

Configure the host with the equivalent command and arguments:

```json
{
  "command": "grimoire",
  "args": ["mcp", "--root", "/absolute/path/to/repository"]
}
```

The surrounding MCP configuration object is host-specific. Keep the repository path absolute when the host may launch the process from another working directory.

The server exposes:

```text
grimoire_discover
```

The installed skill and MCP tool are complementary:

- the skill teaches the agent when and how to use Grimoire efficiently;
- the MCP server executes the unified discovery contract;
- normal shell, Git, search, and file-reading tools remain available for direct verification.

Do not restrict an agent to Grimoire alone. The intended operating model is Grimoire-assisted repository inspection.

## First MCP request

Use a concrete question, choose narrow or balanced breadth deliberately, and reuse one investigation session:

```json
{
  "schema": "grimoire.discovery.v1",
  "mode": "search",
  "root": "/absolute/path/to/repository",
  "query": "camera visibility network interest realtime snapshot",
  "breadth": "balanced",
  "limit": 6,
  "code_only": true,
  "state_mode": "refresh-if-needed",
  "session": "network-interest"
}
```

Use returned handles for follow-ups:

```json
{
  "schema": "grimoire.discovery.v1",
  "mode": "inspect",
  "root": "/absolute/path/to/repository",
  "handles": ["<returned-handle>"],
  "adjacent_context": 3,
  "state_mode": "current-only",
  "session": "network-interest"
}
```

The agent should switch to `current-only` once prepared state is known to match the checkout. Use `refresh-if-needed` after repository changes. Reserve `force-refresh` for explicit rebuilds or demonstrated state corruption.

## Build from source

Requirements:

- Python 3.12 or newer;
- Go 1.26.5;
- Rust 1.90 or newer;
- Node.js 22 for the TypeScript adapter;
- Lodestone checked out beside Grimoire at `../lodestone`, or `LODESTONE_ROOT` set to another checkout.

Expected layout:

```text
workspace/
  grimoire/
  lodestone/
```

Build from the Grimoire root:

```bash
python scripts/workflow.py build --version 0.1.0-dev
```

Install the resulting combined build:

```bash
python scripts/workflow.py install --source build --bin-dir /path/on/your/PATH
```

The source installer accepts the same `--component`, `--skills-dir`, and `--skip-skills` options as the bundled installer.

Run verification:

```bash
python scripts/workflow.py smoke
python scripts/workflow.py test
```

The complete workflow defaults to one worker across Go and Cargo. Use `--jobs N` only when additional concurrency is intentional.

## Optional document vectors

Exact source, BM25 source, symbols, relationships, trace, and impact do not require embeddings.

Optional document vectors require a compatible embedding service:

```bash
grimoire model setup
grimoire model start
grimoire vector build --root .
grimoire search --root . --query "Why is match state authoritative?" --document-vectors
```

`model setup` currently manages Windows x64 artifacts. Other platforms require an externally configured compatible endpoint. See [Embedding model](embedding-model.md) and [Current limitations](../limits/current-limitations.md).

## Failure and recovery

### Command not found

The installer does not edit `PATH`. Confirm the selected binary directory is present in the current shell's `PATH`, then start a new shell.

### Lexicon or Arcana check fails

Install the combined bundle or ensure the provider executables are beside Grimoire or on `PATH`. Controlled environments may set `GRIMOIRE_LEXICON_COMMAND` or `GRIMOIRE_ARCANA_COMMAND`.

### Agent does not load the skill

Confirm one of these files exists and start a new agent session:

```text
~/.agents/skills/grimoire/SKILL.md
~/.hermes/skills/grimoire/SKILL.md
```

For another host, install the skill into that host's documented skill root with `--skills-dir`.

### MCP starts in the wrong repository

Use an absolute `--root` path in the MCP command. Do not rely on the agent host's process working directory.

### First query is unexpectedly slow

Inspect `preparation` and `warnings` in the response. Initial source, Lexicon, Arcana, or documentation preparation can dominate a first query. Reuse current prepared state and avoid routine `force-refresh` calls.

### Results are too large

Use `breadth: "narrow"` for localized work, keep its default four-result combined budget, set `code_only: true`, reuse one `session`, and inspect handles instead of requesting inline evidence or repeating searches.

### Missing evidence or negative claims

Check `warnings`, `preparation`, and `truncated_lanes`. Expand only the relevant lane or verify the bounded claim with ordinary repository search and exact source reads.

## Related docs

- [Agent and MCP guide](agent-mcp.md)
- [Unified discovery contract](agent-query.md)
- [CLI reference](cli.md)
- [Release workflow](../development/release-workflow.md)
- [Current limitations](../limits/current-limitations.md)

## Notes

Installation success requires both executable verification and a representative repository-preparation or MCP smoke test.
