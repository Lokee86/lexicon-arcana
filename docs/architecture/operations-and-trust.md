# Operations and trust boundaries

Parent index: [Architecture](INDEX.md)

## Purpose

This document defines Grimoire's operational ownership, process trust assumptions, request lifecycle, diagnostics, and cross-repository dependency controls.

## Overview

Grimoire is a local CLI and stdio MCP product that coordinates separately owned Lexicon, Arcana, embedding, and Lodestone boundaries. Local execution does not remove the need for explicit trust, timeout, cancellation, admission, diagnostic, and source-identity rules.

## Process trust

The Grimoire, Lexicon, and Arcana executables are trusted local programs. Command overrides such as `lexicon_command` and `arcana_command` execute the selected binary directly without a shell. Operators must not point them at executables supplied by an untrusted repository.

Lexicon consumer definitions are trusted operator configuration stored under the Lexicon state root. Consumers execute directly without a shell, inherit the operator environment, receive repository and snapshot paths, and can read or modify anything allowed to the current account. They are not a sandbox or plugin-security boundary.

Configured embedding endpoints receive text selected for embedding. Use only an endpoint whose data handling is acceptable for the repository being analyzed.

## MCP lifecycle and admission

The stdio MCP server supports exactly the protocol version declared by `mcpserver.ProtocolVersion`. Initialization with another version is rejected and reports the supported version; the server does not infer compatibility from message shape.

Tool calls have independent request contexts. `notifications/cancelled` cancels the matching active request. Admission is bounded by `--max-in-flight`, which defaults to eight; excess calls receive a stable server-busy JSON-RPC error instead of creating an unbounded queue.

The resident discovery runtime serializes access to one decoded snapshot and one Arcana protocol session. Cancellation closes an interrupted Arcana session so the next request reopens a known state rather than continuing an uncertain stream.

## External execution limits

- Grimoire discovery has an owned overall timeout supplied by the CLI or host.
- Arcana process calls inherit request cancellation and are terminated when their request is cancelled.
- Lexicon consumers default to a 30-minute timeout when no timeout is authored; an explicit positive duration may narrow or extend that limit.
- Build and test workflows default to one worker and require an explicit higher job count.

Nested retries are not used for discovery. Provider failure degrades the owning evidence lane and is surfaced through warnings.

## Diagnostics and sensitive data

MCP request IDs and Arcana protocol request IDs provide correlation at the process boundary. Optional Arcana timing diagnostics are structured JSON records carrying a schema, event, request ID, operation, and snapshot identity.

`--audit-log` creates a private JSONL file using schema `grimoire.mcp.audit.v2`. Source and document excerpts are redacted by default. `--audit-include-content` explicitly opts into recording those contents. Audit records still contain query metadata, repository paths, handles, and errors; they must be treated as sensitive local diagnostic data.

Audit retention, rotation, and deletion are operator-owned. Grimoire does not upload audit records or remove them automatically.

## Cross-repository Lodestone boundary

Lodestone owns native vector persistence and exact search. Grimoire pins both the Go module pseudo-version and the exact Lodestone source commit used to build the native library. The root workflow verifies the sibling checkout before tests or release builds, and the release workflow checks out that exact commit.

The checked-in `replace` directive is a local-development override, not an independent source authority. It must resolve to the pinned checkout. This exception remains until Lodestone publishes a tagged Go-module/native-library release that can replace the source checkout without weakening reproducibility.

## Recovery and operator action

Generated source, Lexicon, Arcana, and vector state are reconstructable from repository source, documentation, configuration, and the pinned toolchain inputs. Corrupt or mismatched snapshots are rejected rather than repaired in place.

Operators recover by inspecting status, removing only the affected generated state root when necessary, rebuilding that owner, and rerunning the request. Private state roots are never user-authored authorities.

## Code map

| Boundary | Primary implementation | Protecting tests or gates |
| --- | --- | --- |
| MCP negotiation, admission, cancellation, and framing | `internal/mcpserver/`, `internal/app/mcp.go` | `internal/mcpserver/server_test.go`, `internal/app/mcp_test.go` |
| MCP audit privacy | `internal/app/mcp_audit.go` | `internal/app/mcp_audit_test.go` |
| Arcana process lifecycle and correlation | `internal/arcanagraph/session.go`, `internal/arcanagraph/protocol.go` | `internal/arcanagraph/*_test.go` |
| Lexicon consumer execution limits | `lexicon/src/consumer/` | `lexicon/tests/consumer_execution.rs` |
| Lodestone source identity | `go.mod`, `scripts/workflow.py`, `.github/workflows/release.yml` | `tools/pitlord/repository.json`, `scripts/test_workflow.py`, release verification |
| Generated-state traversal exclusions | `.gitignore`, `internal/index/exclusions.go` | `tools/pitlord/repository.json`, `internal/app/index_exclude_test.go`, `internal/index/*_test.go` |

## Tests

The MCP server tests protect version rejection, bounded admission, cancellation, framing, and error behavior. MCP audit tests protect default content redaction and explicit opt-in recording. Lexicon consumer tests protect the owned default timeout. Pitlord protects dependency direction, generated-state ownership, ADR presence, component boundaries, and the required verification seams for Lodestone source identity.

## Related docs

- [Component architecture](components.md)
- [System overview](system-overview.md)
- [Grimoire MCP interface](../reference/agent-mcp.md)
- [Architecture verification](../development/architecture-verification.md)
- [Current limitations](../limits/current-limitations.md)

## Notes

Local execution is a deployment property, not a security guarantee. Any configured executable or endpoint with access to repository content is inside the operator's trust boundary.
