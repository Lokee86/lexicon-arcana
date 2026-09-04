# ADR 0002: Progressive discovery instead of context packages

Parent index: [Architecture decisions](INDEX.md)

## Purpose

Record why Grimoire exposes progressive evidence discovery and stable-handle expansion instead of preassembled context packages.

## Overview

Independent evidence lanes preserve provenance, degradation, and exact follow-up inspection while avoiding premature global ranking and token fitting.

## Status

Superseded by [ADR 0006](0006-retire-grimoire-lead-with-lexicon-arcana.md) as an active product path. Retained as historical rationale for the retired Grimoire discovery layer.

## Context

Repository investigations combine heterogeneous evidence: exact source matches, ranked source ranges, documentation, language symbols, and graph relationships. Preassembling and token-fitting all potentially relevant evidence forces ranking and scope decisions before the consumer knows which thread matters.

Canonical current architecture: [System overview](../architecture/system-overview.md) and [Unified discovery contract](../reference/agent-query.md).

## Decision

Use progressive discovery through `grimoire.discovery.v1`:

- preserve exact, source, document, symbol, and relationship evidence as independent lanes;
- return stable snapshot-qualified handles for follow-up work;
- use `search` or `orient` for discovery, `inspect` for exact evidence, and bounded `trace` or `impact` for structural expansion;
- provide balanced per-lane budgets and a compact narrow mode with one combined code-evidence budget;
- let investigation sessions replace repeated evidence with prior handles;
- require consumers to verify material conclusions against exact source.

The former context-package compiler and token-fitted assembly path are retired.

## Ownership and dependency consequences

- `internal/agentquery` owns provider-neutral discovery and expansion behavior.
- `internal/agentruntime` combines independently owned source, document, symbol, relationship, and session results.
- Retrieval providers retain their own ranking semantics; no provider owns a global answer score.
- Grimoire selects and coordinates Lexicon and Arcana providers. Consumers do not choose or combine provider wire contracts.
- Documentation remains a separate evidence class and cannot displace implementation evidence in source lanes.

## State, lifecycle, and failure consequences

- Discovery prepares or inspects repository state according to `current-only`, `refresh-if-needed`, or `force-refresh` policy.
- Handles bind follow-up operations to the snapshot that produced them; stale handles fail rather than triggering fuzzy rediscovery.
- Missing Arcana, Lexicon, or document-vector state degrades only affected lanes when unrelated evidence remains valid.
- Warnings and truncated-lane metadata make bounded or degraded results visible.
- Sessions persist returned evidence, not agent reasoning.

## Alternatives considered

- Build a complete token-fitted context package before investigation. Rejected because it predicts expansion too early and merges unlike evidence.
- Produce one global ranking across all providers. Rejected because scores and authority differ by evidence class.
- Make embeddings the required first-stage retriever. Rejected because exact and BM25 source discovery must remain deterministic and independently available.
- Return full source for every search hit. Rejected as the default because handle-first discovery avoids unnecessary expansion.

## Compatibility and migration impact

`grimoire query <mode>` remains a compatibility spelling for the direct progressive commands. New integrations should use the direct commands and `grimoire.discovery.v1`. Removed context-package commands and assembly contracts are not compatibility targets; historical package evaluations remain calibration records only.

## Verification

This decision is protected by agent-query, agent-runtime, evidence, MCP, handle, session, retrieval, provider-degradation, and end-to-end agent-discovery tests. The contract matrix specifically verifies independent lanes, bounded handle-first narrow discovery, stable handles, deterministic ordering, and embedding-independent source search.

See [Testing and benchmarks](../development/testing-and-benchmarks.md) and the [Behavioral contract matrix](../development/behavioral-contract-matrix.md).

## Risks and debt

- Consumers must choose among lanes because there is no global answer ranking.
- Bounded search can omit relevant evidence; warnings, `truncated_lanes`, and targeted expansion must be respected.
- Progressive interaction can cost more than direct file inspection for exact lookups or short call chains.
- Retrieval quality and measured benefit remain corpus- and task-dependent.

## Superseding conditions

Supersede this ADR only if measured evidence supports a replacement investigation contract that preserves evidence provenance, deterministic behavior, exact verification, bounded expansion, and provider degradation. Reintroducing preassembled packages or global ranking requires an explicit replacement decision and migration path.

## Related docs

- [System overview](../architecture/system-overview.md)
- [Unified discovery contract](../reference/agent-query.md)
- [Retrieval quality](../development/retrieval-quality.md)

## Notes

Canonical discovery documentation defines current fields and behavior; this ADR defines why the interaction model exists.
