# ADR 0004: Process and protocol boundaries between components

Parent index: [Architecture decisions](INDEX.md)

## Purpose

Record why Grimoire integrates Lexicon and Arcana through immutable data and explicit process protocols rather than source-level ownership sharing.

## Overview

Process and protocol seams preserve component lifecycle, failure attribution, independent use, and language-runtime boundaries.

## Status

Partially superseded by [ADR 0006](0006-retire-grimoire-lead-with-lexicon-arcana.md). Lexicon snapshot publication and Arcana process/protocol ownership remain current; Grimoire forwarding, discovery, and response-shaping ownership is retired.

## Context

Grimoire coordinates Go-based language analysis, Rust-based graph analysis, and product-level discovery while preserving independent component operation. Direct library coupling would merge release, runtime, and failure domains and would make it easier for one component to depend on another's internals.

Canonical current architecture: [Component architecture](../architecture/components.md) and [Analysis stack](../architecture/analysis-stack.md).

## Decision

Integrate independently owned engines across explicit process, export, and protocol boundaries:

- Lexicon publishes immutable normalized facts and snapshots for consumers;
- Arcana consumes one verified Lexicon snapshot and serves graph behavior through its owned CLI and line-oriented protocol;
- Grimoire reads Lexicon exports through `internal/lexiconfacts` and queries Arcana through `internal/arcanagraph`;
- Grimoire's `lexicon` and `arcana` command namespaces forward specialist commands to the owning executables, preserving stdin, stdout, stderr, and exit status;
- product-facing responses are provider-neutral and do not expose Lexicon or Arcana wire types.

No component may acquire ownership by parsing another component's private files or reimplementing its behavior at the Grimoire boundary.

## Ownership and dependency consequences

- Lexicon owns adapter execution, normalized fact schemas, and snapshot publication.
- Arcana owns graph ingestion, validation, storage, query semantics, and protocol compatibility.
- Grimoire owns executable discovery, orchestration, timeout/fallback policy, protocol clients, and public response shaping.
- Dependency direction remains Lexicon → Arcana, with Grimoire consuming both; neither provider calls back into Grimoire for deterministic analysis.
- Changes to an exported schema or protocol require coordinated producer, consumer, documentation, and test updates while preserving the owning implementation.

## State, lifecycle, and failure consequences

- Provider processes retain separate startup, locking, publication, and exit behavior.
- Namespaced forwarding reports native failures instead of translating them into invented Grimoire semantics.
- Discovery timeouts, missing executables, stale state, protocol errors, or incompatible providers produce explicit warnings or failures at the affected boundary.
- Arcana failure may fall back to direct Lexicon relationships; missing structural providers do not invalidate healthy source and document lanes.
- Grimoire never repairs provider state by mutating it directly; it invokes the owning lifecycle.

## Alternatives considered

- Link Lexicon and Arcana internals directly into Grimoire. Rejected because it collapses ownership, language/runtime, and release boundaries.
- Parse provider stdout or private state ad hoc at each call site. Rejected because it creates unstable implicit contracts.
- Duplicate parser or graph behavior in Grimoire as fallback. Rejected because it creates competing semantic owners.
- Require providers to run as supervised long-lived daemons. Rejected as the current default because standalone commands and request-driven preparation are sufficient and independently operable.

## Compatibility and migration impact

Provider command discovery may resolve bundled, configured, or directly invoked executables. Forwarded arguments and native exit behavior remain component-owned. Export and protocol version changes must be rejected or explicitly adapted; field presence alone is not compatibility. Consolidated packaging does not convert process boundaries into internal Go or Rust APIs.

## Verification

This decision is protected by Grimoire engine-command forwarding, provider discovery, repository preparation, Lexicon export, Arcana ingestion/protocol, timeout, degradation, and public response-shaping tests. Combined installation smoke tests verify that all three binaries remain runnable.

See the [Grimoire CLI reference](../reference/cli.md) and [Behavioral contract matrix](../development/behavioral-contract-matrix.md).

## Risks and debt

- Process startup, serialization, and protocol translation add latency and diagnostic surface.
- Version skew between executables and state formats must remain loud and testable.
- Platform-specific executable discovery can fail even when component code is valid.
- Fallback evidence can be narrower than Arcana-backed graph evidence.

## Superseding conditions

Supersede this ADR only if component independence is intentionally removed or a new boundary provides equivalent ownership clarity, native failure attribution, version negotiation, independent operation, and lane-tolerant degradation. A shared library or daemon model requires its own lifecycle and migration decision.

## Related docs

- [Component architecture](../architecture/components.md)
- [Operations and trust boundaries](../architecture/operations-and-trust.md)
- [Grimoire MCP interface](../reference/agent-mcp.md)

## Notes

Transport convenience must not move parsing, graph, or state authority into the coordinating component.
