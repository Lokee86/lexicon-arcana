# Documentation Policy

Parent index: [Lexicon + Arcana documentation](INDEX.md)

## Purpose

Define documentation ownership and minimum coverage for the Lexicon + Arcana product family and its retained historical Grimoire evidence.

## Overview

The root documentation tree owns shared L+A architecture, decisions, release/installation composition, benchmark evidence, limitations, planning, and documentation governance. Lexicon and Arcana retain independently indexed component documentation under their source roots.

Historical Grimoire pages may remain where they are necessary to interpret prior ADRs, reports, fixtures, and benchmarks, but they must be clearly distinguishable from current contracts.

## Canonical ownership

| Information | Canonical owner |
| --- | --- |
| Product-family introduction and shared distribution | `README.md` and `docs/reference/installation.md` |
| Shared L+A architecture and consumer boundaries | `docs/architecture/analysis-stack.md`, `components.md`, `system-overview.md` |
| Architecture decisions and retirement rationale | `docs/decisions/` |
| Shared tests, benchmarks, release workflow, coverage, and behavioral contracts | `docs/development/` |
| Current product-family limitations | `docs/limits/` |
| Future and unresolved work | `docs/planning/` |
| Lexicon behavior/contracts | `lexicon/docs/`, `lexicon/spec/` |
| Arcana behavior/contracts | `arcana/docs/` |
| Stable repository operating rules | `AGENTS.md` |

## Rules

- Current root reference pages describe implemented L+A behavior only.
- Historical Grimoire pages are evidence, not current implementation contracts.
- Architecture pages identify implemented ownership, non-ownership, state, lifecycle, failure, and recovery boundaries.
- Component-specific behavior belongs in the owning component documentation and is summarized rather than duplicated at the root.
- Retiring a capability does not transfer its ownership to Lexicon or Arcana by default.
- Benchmark/research claims name corpus, method, artifacts, limitations, task scope, model/condition, and date where applicable.
- Planning pages never serve as current implementation reference.
- Limitations remain explicit until resolved.
- Every active command family, component boundary, stateful flow, and machine-readable contract maps to current documentation.
- Documentation baselines are not permitted; shared, Lexicon, and Arcana checks must pass without suppressed findings.
- Pitlord owns executable repository architecture policy; shared documentation tooling owns documentation structure and change impact.

## Code map policy

Current implementation-facing architecture, reference, development, contract, pipeline, and adapter documents include focused `## Code map` sections when required by the configured checker. Historical Grimoire pages are not required to remain synchronized with deleted implementation paths.

A focused code map identifies the primary implementation owner, related state/artifacts, protecting tests/gates, and important non-ownership boundaries.

## Required change impact

Changes to Lexicon semantics/publication, Arcana ingestion/graph/protocol, shared release/install behavior, product-family architecture, or active downstream-consumer contracts require updates to the exact owner in the same change.

Changes that retire an interface must also update documentation indexes/governance so historical material is not presented as current behavior.

## Related docs

- [Documentation procedure](documentation-procedure.md)
- [Documentation coverage](development/documentation-coverage.md)
- [Component architecture](architecture/components.md)
- [Shared documentation standard](../.standards/docs/documentation-standard.md)

## Notes

The combined distribution is not a third runtime product. Documentation must preserve the Lexicon, Arcana, shared-release, and downstream-consumer ownership boundaries established by ADR 0006.
