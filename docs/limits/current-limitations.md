# Current limitations

Parent index: [Limits](INDEX.md)

## Purpose

Record current limitations of the active Lexicon + Arcana product family and the unfinished retirement transition.

## Overview

These constraints describe implemented behavior, not proposed remedies. Grimoire-specific runtime limitations are historical after ADR 0006 and no longer belong in the active limitations list.

## Repository identity is still transitional

The source repository is still named `grimoire` even though the active products are Lexicon + Arcana. Current build and release artifacts use L+A naming, but repository/package/mirror naming has not completed its final migration.

## Production agent guidance is not shipped yet

The checked-in L+A skill under `evaluation/` is benchmark-oriented and may assume frozen exports/snapshots. The active release does not yet install a production L+A agent skill or define a final agent-host discovery convention.

Consumers should currently use direct source/Git tools plus the documented Lexicon and Arcana command/protocol surfaces.

## Lexicon coverage remains language- and construct-dependent

Lexicon semantic quality is bounded by its enabled adapters and supported constructs. Unsupported or ambiguous relationships should remain unresolved rather than being guessed. Cross-language semantic completeness is not guaranteed merely because a repository scans successfully.

## Arcana graph completeness depends on Lexicon evidence

Arcana can only compile relationships represented by the consumed Lexicon snapshot or by Arcana-owned deterministic graph semantics. Missing or unresolved Lexicon evidence can therefore produce an incomplete graph without making the graph corrupt.

Unknown relation semantics must not be invented for compatibility.

## Arcana semantic vectors are optional external-service features

Exact graph traversal does not require embeddings. Optional semantic graph entry points require a compatible external embedding endpoint and are subject to that service's availability, model identity, latency, and resource limits.

The active release does not ship or supervise a Grimoire/Lodestone embedding runtime.

## Combined distribution is not a third product

The root workflow packages Lexicon and Arcana together for convenience, but there is no umbrella runtime. Consumers that require orchestration, context routing, stopping policy, or probabilistic task decomposition need a downstream owner such as Warlock.

## Benchmark evidence is task-shaped

Current agent benchmarks do not prove universal benefit. L+A can reduce independent repository exploration on some diagnosis tasks, while direct source search may remain cheaper for exact lookups or already-known symbols.

Benchmark claims must retain model, task, repository revision, prompt condition, grounding, and date.

## Historical Grimoire material remains in the repository

Historical ADRs, benchmark results, reports, evaluation fixtures, and some reference pages intentionally retain Grimoire names. They are evidence for prior experiments, not active implementation contracts.

During retirement cleanup, tooling and documentation must distinguish historical references from active dependencies rather than deleting evidence indiscriminately.

## Release workflow is deliberately bounded

The root workflow defaults to one worker across Go and Cargo. `--jobs N` is an explicit operator choice and can overload a machine when set too high.

## Compatibility is pre-stable

Lexicon and Arcana command spelling, diagnostic codes, exit classes, state migration policy, and packaging conventions are not yet stable-release promises unless their component documentation states otherwise.

## Related docs

- [System overview](../architecture/system-overview.md)
- [Roadmap](../planning/roadmap.md)
- [Agent benchmark findings](../development/agent-benchmark-findings.md)
- [Lexicon documentation](../../lexicon/docs/README.md)
- [Arcana documentation](../../arcana/docs/README.md)

## Notes

Resolved limitations should be removed or rewritten in the same change that establishes the new current behavior. Historical Grimoire limitations belong in historical reports, not here.
