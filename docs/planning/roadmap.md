# Roadmap

Parent index: [Planning](INDEX.md)

## Purpose

Own the remaining Lexicon + Arcana transition and product work after Grimoire retirement.

## Overview

Grimoire's discovery runtime, MCP surface, repository state, installed skill, and active release ownership are retired. Remaining work is about finishing the L+A product transition, improving measured repository-analysis value, and integrating the surviving components cleanly with downstream consumers such as Warlock.

## Current status

Retirement implementation is in progress. Active build/test/install/release already targets Lexicon + Arcana only. The retired Grimoire source/runtime is being removed while historical benchmark evidence and ADRs remain preserved.

## Expected ownership

- Lexicon owns polyglot semantic extraction, normalized facts, immutable semantic snapshots, and publication lifecycle.
- Arcana owns verified Lexicon ingestion, repository/call graph storage, graph queries, snapshots, overlays, and optional semantic graph entry points.
- Warlock/consumers own agent/task/context orchestration.
- Ordinary source/Git/search tools remain direct evidence surfaces.
- No surviving component owns Grimoire's retired umbrella discovery layer.

## Planned behavior

The product family should ship and operate as two deterministic analysis tools with a small shared distribution surface, direct consumer boundaries, preserved benchmark evidence, and no hidden dependency on retired Grimoire code or state.

## Implementation sequence

1. Finish physical Grimoire runtime and product-specific dependency removal while preserving historical evidence.
2. Remove or reclassify stale Grimoire-only documentation, governance, and evaluation entry points.
3. Update Warlock and other downstream consumers to discover/use Lexicon and Arcana directly.
4. Rename repository/release identity away from Grimoire when migration compatibility no longer benefits from the old repository name.
5. Continue judged repository-analysis experiments on larger and more varied corpora.

## Near-term priorities

- Complete the runtime/source deletion and verify no active build, release, CI, policy, or documentation owner still requires Grimoire.
- Wire Warlock to the direct Lexicon/Arcana surfaces and installed-state conventions now used by the production skill.
- Preserve the clean distinction between the installed production skill and frozen benchmark prompt experiments.
- Add stable machine-readable diagnostics/exit classes where current component behavior remains pre-release.
- Expand judged corpora across languages, repository sizes, and task classes.

## Lexicon work

- Continue semantic-fact coverage where judged consumers need richer language semantics.
- Improve adapter correctness and unresolved-evidence quality before adding speculative cross-language inference.
- Measure initialization and incremental scan cost on substantially larger repositories.
- Keep immutable publication and bounded external-consumer behavior as hard contracts.

## Arcana work

- Continue graph correctness, compatibility, overlay, compaction, and query-protocol validation.
- Evaluate community/summary/semantic entry points only where they reduce real investigation cost.
- Keep exact graph traversal authoritative and semantic vectors optional.
- Measure storage and traversal behavior at larger graph scales.

## Agent and consumer work

- Maintain the shipped L+A production skill as bounded direct-component guidance; keep the benchmark skill frozen to its experiment conditions.
- Keep source inspection as implementation authority.
- Use Lexicon for semantic ownership/relationships and Arcana for bounded graph questions.
- Put stopping, task decomposition, context routing, and probabilistic continuation in Warlock rather than either analysis component.
- Benchmark Plain versus L+A as the normal repository-agent comparison; retain Grimoire runs only as historical baselines.

## Distribution and compatibility

- Decide final repository and bundle naming after retirement cleanup.
- Define canonical install/discovery locations for Lexicon executable, adapters, and Arcana executable.
- Decide whether former component repositories become mirrors or redirects.
- Define stable migration policy for `.lexicon/` and `.arcana/` before a stable release.
- Keep `.grimoire/` historical/migration-only; do not invent new state there.

## Release gates

Establish measured gates for:

- Lexicon adapter and publication correctness;
- Arcana graph correctness and Lexicon compatibility;
- `arcana.query.v1` compatibility;
- independent Lexicon and Arcana installation;
- combined L+A bundle integrity;
- end-to-end agent correctness, grounding, and investigation efficiency;
- preparation latency and memory on representative repository scales;
- documentation and architecture-policy compliance.

## Acceptance criteria

A roadmap item requires a named owner, implementation plan, focused tests/evaluation, documentation impact, and a clear current-behavior owner before it is complete.

Grimoire retirement is complete when:

- no active build/release/CI surface requires or publishes Grimoire;
- retired runtime/MCP/discovery source is absent from the active tree;
- current documentation and policy describe only L+A ownership;
- Lexicon and Arcana build/test independently and through the combined workflow;
- downstream current products consume direct component boundaries;
- historical benchmark artifacts/ADRs remain understandable and intact.

## Open decisions

Open decisions are the final repository/product-family naming, downstream Warlock integration shape, and which larger-repository tasks justify additional Lexicon/Arcana capabilities.

## Related docs

- [System overview](../architecture/system-overview.md)
- [Current limitations](../limits/current-limitations.md)
- [Agent benchmark findings](../development/agent-benchmark-findings.md)
- [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md)

## Notes

A future capability must have an explicit Lexicon, Arcana, shared-release, or downstream-consumer owner. Do not restore retired Grimoire responsibilities as an implicit coordination layer.
