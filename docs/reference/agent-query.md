# Unified discovery contract

Parent index: [Reference](INDEX.md)

Historical status: this page describes the retired `grimoire.discovery.v1` contract and is retained only for prior benchmark/design interpretation. It is not an active product contract.

## Purpose

This document defines the exact `grimoire.discovery.v1` request, response, evidence-lane, handle, warning, and progressive-expansion contract.

## Overview

Consumers submit orient, search, inspect, trace, or impact operations to one provider-neutral interface. Grimoire preserves provenance and independent lane semantics while coordinating current source, documentation, Lexicon, and Arcana state.

Grimoire exposes one progressive repository-discovery interface over prepared source, repository documentation, Lexicon symbols, and explicit Arcana-backed structural expansion. Consumers do not select a provider. Grimoire routes each operation internally and returns provider provenance with every result.

The schema is `grimoire.discovery.v1`.

## Modes

```bash
grimoire orient --root .
grimoire search --root . --query "SubmitLogin"
grimoire trace --root . --anchor "<handle>" --depth 3
grimoire impact --root . --anchor "<handle>" --direction incoming
grimoire inspect --root . --handle "<handle>" --adjacent-context 3
```

`grimoire query <mode>` remains a compatibility spelling for the same interface.

- `orient` returns compact source and symbol anchors plus suggested expansions.
- `search` returns ranked discovery evidence. Balanced search preserves independent exact, source, document, and symbol budgets; narrow search applies one combined code-evidence budget and defers expansion.
- `trace` expands one stable structural handle through bounded paths.
- `impact` performs bounded incoming, outgoing, or bidirectional traversal.
- `inspect` reads exact source or document evidence by stable handle. Supplied handles are never fuzzily rediscovered.

## Search lanes

A search response may contain:

| Field | Meaning |
| --- | --- |
| `exact_matches` | Literal source matches for concrete identifiers, paths, routes, and configuration keys |
| `source_matches` | BM25-ranked implementation ranges |
| `document_matches` | Separately indexed documentation sections, including text, line ranges, freshness metadata, reasons, and code links |
| `symbol_matches` | Lexicon-grounded declarations and definitions |

With `breadth: "balanced"`, `limit` applies independently to each discovery lane. A full exact lane does not suppress source, symbol, or document results. With `breadth: "narrow"`, one combined limit is round-robin allocated across exact, symbol, and source evidence while overlapping cross-lane representations are suppressed. Narrow search defaults to four combined results.

Narrow search also defaults to `detail: "handles"`: results retain stable inspectable handles, paths, names, kinds, ranks, and reasons while inline excerpts and duplicated node spans are deferred to `inspect`. `truncated_lanes` identifies ranked evidence that remains available beyond the returned budget.

Documentation never appears in `exact_matches`, `source_matches`, or `symbol_matches`. Use `--code-only` or `include_documents: false` to omit the document lane entirely.

## Source and documentation semantics

Source and documentation are separate evidence classes:

- Source describes current executable behavior.
- Documentation describes intent, rationale, constraints, plans, or historical decisions.

A document result may be relevant while stale. Its path, line range, commit metadata, reasons, and stable `knowledge://` handle remain visible so the consumer can assess it independently rather than allowing it to displace implementation evidence.

## Deferred structural expansion

Search does not automatically traverse graph neighbours. When inspectable discovery handles are returned, `deferred_expansions` identifies `trace` and `impact` as explicit structural follow-ups.

`trace` returns query-ranked bounded paths. `impact` requests a larger bounded candidate set from both Lexicon and Arcana, merges duplicate semantic dependents across providers, and returns one ranked list. Each dependent may include:

- `rank` and task-local `score`;
- depth, direction, typed relation, and certainty;
- a stable node handle and provider provenance;
- reasons explaining query relevance, production-versus-test treatment, depth, and certainty;
- source spans and relationship evidence when available.

Impact scores are local to one impact response. They are not calibrated against search-lane scores.

## Request fields

A complete request may be supplied as JSON:

```bash
grimoire query --request '{"schema":"grimoire.discovery.v1","mode":"search","root":".","query":"SubmitLogin","limit":8}'
```

Common fields:

- `mode`, `root`, `state`, and `state_mode`;
- `query`, `anchor`, `target`, and `handles`;
- `limit`, `depth`, `direction`, and `relations`;
- `adjacent_context`, `detail`, and search `breadth`;
- `code_only`, `include_documents`, and `use_document_vectors`;
- optional repository-provider state or executable overrides.

Public CLI and MCP search default to 12 results per lane in balanced mode and four combined results in narrow mode. Trace defaults to eight. `detail` accepts `handles`, `summary`, or `full`; narrow search defaults to `handles`. `limit` is bounded to 200, `depth` to 16, and adjacent inspection context to 200 lines.

## Handles

Every source range and structural node includes a stable snapshot-qualified handle. Document sections expose `knowledge://` handles.

- Source handles identify an exact prepared-index range.
- Lexicon handles identify a durable symbol in one immutable Lexicon snapshot.
- Arcana handles identify a durable node plus its snapshot-local graph ID.
- Document handles identify one exact indexed section.

Follow-up operations should use handles rather than repeating a broad search. Handles are rejected when their snapshot no longer matches active repository state.

## Other response fields

All responses include `schema`, `mode`, and `snapshot`. Depending on mode they may also contain:

- `paths` and `unresolved` for trace;
- `dependents` for impact;
- `inspections` for source inspection;
- `document_matches` for document inspection;
- `assessment`, `coverage`, `deferred_expansions`, `suggestions`, `warnings`, `preparation`, and investigation-session `delta` metadata.

`assessment` is conservative workflow guidance. It reports observed and missing `owner`, `control_flow`, `public_boundary`, and `tests` dimensions plus a `status` and `next_action`. It does not claim exhaustive correctness. Narrow session-backed search records discovery nodes and retrieval hits but does not materialize source ranges until `inspect`.

Provider degradation is reported in `warnings`; remaining lanes still return when possible.

## Code map

| Contract area | Primary implementation | Related tests |
| --- | --- | --- |
| Public request and response schema | `internal/agentquery/schema.go`, `model.go` | `internal/agentquery/query_test.go` |
| Orientation and search modes | `internal/agentquery/orient.go`, `search.go`, `search_budget.go` | `internal/agentquery/query_test.go`, response-shaping tests |
| Handle creation and inspection | `internal/agentquery/handle.go`, `inspect.go`, `resolve.go` | query and runtime tests |
| Trace and impact expansion | `internal/agentquery/trace.go`, `trace_shape.go`, `impact.go`, `impact_shape.go` | trace-shaping and impact-shaping tests |
| Evidence diversity, excerpts, and assessment | `internal/agentquery/diversity.go`, `excerpt.go`, `assessment.go` | corresponding `*_test.go` files |
| Lane assembly and budget enforcement | `internal/agentruntime/`, `internal/evidence/` | package-local `*_test.go` files |
| Lexicon and Arcana adapters | `internal/lexiconfacts/`, `internal/arcanagraph/` | provider package tests |

Source, documentation, and symbol discovery lanes remain separate. Structural traversal is an explicit follow-up. This contract does not make Arcana vectors or any single ranking lane authoritative for the final answer.

## Related docs

- [Agent and MCP guide](agent-mcp.md)
- [CLI reference](cli.md)
- [System overview](../architecture/system-overview.md)
- [Current limitations](../limits/current-limitations.md)

## Notes

Assessment fields are conservative workflow guidance and must not be interpreted as proof of repository-wide completeness.
