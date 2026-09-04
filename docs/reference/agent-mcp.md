# Grimoire agent and MCP guide

Parent index: [Reference](INDEX.md)

Historical status: this page describes the retired Grimoire MCP/agent surface and is retained only for prior benchmark/design interpretation. It is not an active product contract.

## Purpose

This document defines how agent hosts install, configure, invoke, and safely use Grimoire's MCP discovery surface.

## Overview

`grimoire mcp` exposes the provider-neutral `grimoire_discover` tool over stdio, including preparation modes, stable handles, session reuse, evidence semantics, degradation, and efficient progressive investigation.

`grimoire mcp` serves the unified discovery contract over stdio:

```bash
grimoire mcp --root /absolute/path/to/repository
```

The server exposes one tool:

```text
grimoire_discover
```

Use an absolute root when an agent host may start the process from another working directory.

## Host configuration

The exact configuration envelope is host-specific, but the process definition is equivalent to:

```json
{
  "command": "grimoire",
  "args": ["mcp", "--root", "/absolute/path/to/repository"]
}
```

Verify the binaries before configuring the host:

```bash
grimoire version
grimoire lexicon check
grimoire arcana check
```

See [Installation and agent setup](installation.md) for release installation, source builds, PATH setup, and troubleshooting.

## Installed agent skill

Historical Grimoire distributions included the canonical `skills/grimoire/SKILL.md`. That source/install surface has been removed; the paths below document the former installation behavior:

```text
~/.agents/skills/grimoire/SKILL.md
~/.hermes/skills/grimoire/SKILL.md
```

Start a new agent session after installation or update so the host can discover the skill.

The skill teaches agents to:

- use direct search and file reads first when exact paths or symbols are known;
- escalate localized uncertainty to `breadth: "narrow"` and distributed investigations to `breadth: "balanced"`;
- reuse one investigation `session`;
- treat narrow search as handle-only discovery;
- disable documents for implementation-only work;
- follow stable handles with `inspect`, and use `trace` or `impact` only for a named unresolved relationship;
- avoid unnecessary state refreshes;
- verify material conclusions against exact source;
- stop querying Grimoire when direct inspection becomes cheaper.

The skill is operating guidance, not a second product interface. The MCP server remains the executable discovery surface.

## Intended tool access

Do not restrict an agent to Grimoire alone. A normal Grimoire-assisted agent should retain:

- shell commands and literal repository search;
- Git inspection;
- direct source reads;
- the installed Grimoire skill;
- the `grimoire_discover` MCP tool.

This is important for both correctness and efficiency. Grimoire should reduce broad discovery work, while exact source reads remain the final proof of implementation claims.

## Normal workflow

1. Use direct search and file reads when the task already names an exact path or symbol.
2. Use one `breadth: "narrow"` search for localized uncertainty, or `breadth: "balanced"` for distributed context.
3. Review `assessment`; inspect selected handles when it reports `ready-for-targeted-inspection`.
4. Use `trace` or `impact` only for a named unresolved relationship.
5. Reuse one short `session` name for the investigation.
6. Stop when `assessment` reports `ready-to-synthesize` or the task's required dimensions are grounded.
7. Check `warnings`, `preparation`, and `truncated_lanes` before completeness or negative claims.

Use `orient` only when the repository is unfamiliar and the task has no useful search terms.

## Efficient first request

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

Balanced search uses independent per-lane limits. Narrow search uses one combined code-evidence limit and defaults to four handle-only results. Do not raise the narrow limit until the default set demonstrably misses a required dimension.

Set either:

```json
{"code_only": true}
```

or:

```json
{"include_documents": false}
```

when documentation is not materially relevant.

## Handle-based follow-ups

Inspect exact evidence:

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

Trace a bounded structural path:

```json
{
  "schema": "grimoire.discovery.v1",
  "mode": "trace",
  "root": "/absolute/path/to/repository",
  "anchor": "<returned-handle>",
  "depth": 3,
  "limit": 6,
  "state_mode": "current-only",
  "session": "network-interest"
}
```

Find bounded dependents:

```json
{
  "schema": "grimoire.discovery.v1",
  "mode": "impact",
  "root": "/absolute/path/to/repository",
  "anchor": "<returned-handle>",
  "direction": "incoming",
  "depth": 3,
  "limit": 6,
  "state_mode": "current-only",
  "session": "network-interest"
}
```

Do not repeat a broad natural-language search when a returned handle already identifies the source or symbol to expand.

## State modes

| Mode | Use |
| --- | --- |
| `current-only` | Prepared state is known to match the checkout; fail rather than refresh |
| `refresh-if-needed` | Normal first request or repository state may have changed |
| `force-refresh` | Explicit rebuild or demonstrated invalid state only |

Initial preparation may dominate the first request. Reuse prepared state across related investigations and report preparation separately in benchmarks.

## Protocol lifecycle

Grimoire supports exactly MCP protocol version `2025-11-25`. Initialization with another version is rejected with `-32602` and a `supported_protocol_version` value; the server does not infer compatibility from field shape.

Tool calls execute with independent request contexts so the server can continue processing pings and cancellation notifications. `notifications/cancelled` cancels the matching request ID. The default maximum is eight accepted calls awaiting completion; `--max-in-flight` changes that bound, and excess calls receive server-busy error `-32001`.

Discovery state itself remains serialized through one resident runtime and Arcana protocol session. Cancellation closes an interrupted Arcana stream before later work reuses it.

## Input

The tool accepts the `grimoire.discovery.v1` fields documented in [Unified discovery contract](agent-query.md), including:

- `mode`, `root`, `state`, and `state_mode`;
- `query`, `anchor`, `target`, and `handles`;
- `limit`, `depth`, `direction`, and `relations`;
- `adjacent_context`, `detail`, and search `breadth`;
- `code_only`, `include_documents`, and `use_document_vectors`;
- `session`;
- controlled provider state or executable overrides.

Agents do not choose between Lexicon and Arcana. Grimoire owns provider routing.

## Output

The response uses the same flattened schema as the CLI:

- `exact_matches`;
- `source_matches`;
- `document_matches`;
- `symbol_matches`;
- `relationship_matches`;
- mode-specific `paths`, `dependents`, or `inspections`;
- `snapshot`, `assessment`, `coverage`, `preparation`, `warnings`, and `truncated_lanes`.

When `session` is supplied, newly discovered evidence is returned through `delta`; repeated evidence is represented by prior handles rather than replayed in full. Narrow search deltas contain discovery nodes and retrieval hits but defer source ranges until `inspect`.

## Evidence semantics

Interpret lanes independently:

- source is authoritative for current implementation behavior;
- documentation supplies intent, rationale, plans, or historical constraints and may be stale;
- symbols identify declarations and definitions;
- relationships identify bounded graph evidence and may degrade when structural providers are unavailable.

Do not count the same underlying source occurrence multiple times merely because it appears in several lanes.

## Completeness and negative claims

Before claiming that a symbol has no callers, a feature has no implementation, or no other relevant files exist:

1. inspect `warnings` and provider degradation;
2. inspect `truncated_lanes`;
3. expand only materially relevant bounded lanes;
4. verify the scope with normal repository search when needed;
5. state unresolved coverage limits rather than presenting partial discovery as exhaustive.

## State preparation and degradation

The MCP runtime aligns Grimoire source, documentation, Lexicon, and Arcana state when `state_mode` permits refresh.

- Missing or failed structural providers are reported as warnings.
- Exact and BM25 source discovery may continue when structural lanes degrade.
- Documentation vectors are optional and validated for freshness.
- Vector state is never required for exact, source, symbol, relationship, trace, or impact operations.

## Choosing Grimoire or direct inspection

Start with direct shell inspection when the task is a literal path, exact identifier, or short call chain with obvious names.

Start with Grimoire when the task requires architectural ownership, cross-language discovery, generated contracts, source-plus-document reasoning, impact analysis, or broad implementation planning.

Do not force a minimum number of Grimoire calls. For narrow work, a third call requires a named unresolved behavior, boundary, test seam, or relationship. An agent that uses one search, one targeted inspection, and then synthesizes is operating correctly.

See [Agent benchmark findings](../development/agent-benchmark-findings.md) for measured examples.

## Code map

| Surface | Primary implementation | Related tests |
| --- | --- | --- |
| MCP command startup and tool registration | `internal/app/mcp.go` | `internal/app/mcp_test.go`, `internal/app/mcp_audit_test.go` |
| Protocol negotiation, bounded admission, and cancellation | `internal/mcpserver/server.go`, `internal/mcpserver/model.go` | `internal/mcpserver/server_test.go` |
| JSON-RPC framing and stdio server | `internal/mcpserver/framing.go`, `model.go`, `server.go` | `internal/mcpserver/server_test.go` |
| Discovery request execution | `internal/agentruntime/`, `internal/agentquery/` | package-local `*_test.go` files |
| Stable handles and session reuse | `internal/investigation/`, `internal/agentruntime/session_handles.go` | `internal/investigation/*_test.go`, `internal/agentruntime/*_test.go` |
| Repository preparation and state modes | `internal/repostate/`, `internal/app/discovery_prepare.go` | `internal/repostate/*_test.go`, `internal/app/discovery_test.go` |
| Installed agent-skill packaging | `scripts/install.py`, packaged skill assets | installation and MCP smoke tests |

The MCP layer exposes Grimoire's provider-neutral discovery surface. It does not expose Lexicon or Arcana internals as direct wire contracts.

## Related docs

- [Installation and agent setup](installation.md)
- [Unified discovery contract](agent-query.md)
- [CLI reference](cli.md)
- [Agent benchmark findings](../development/agent-benchmark-findings.md)

## Notes

The MCP boundary exposes Grimoire's public discovery contract, not Lexicon or Arcana internal wire types.
