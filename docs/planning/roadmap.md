# Roadmap

Parent index: [Planning](INDEX.md)

## Purpose

This document owns future, unresolved, and sequenced work for Grimoire's progressive repository-discovery product.

## Overview

Implemented behavior is documented in current architecture and reference owners. This roadmap retains only work that is incomplete, under investigation, or awaiting an explicit implementation decision.

## Current status

Active planning. The completed-foundation list is retained only to establish the baseline from which remaining work is sequenced.

## Expected ownership

Grimoire owns discovery orchestration, source and documentation retrieval, handles, sessions, and state coordination. Lexicon owns language analysis, Arcana owns graph behavior, and Lodestone owns native vector storage. Planned work must enter through the appropriate existing seam or explicitly establish a new owner.

## Planned behavior

The remaining work improves measured agent outcomes, preparation efficiency, evidence quality, vector portability, diagnostics, distribution, and compatibility without reviving the retired preassembled-context pipeline.

## Implementation sequence

1. Close protocol, warning-propagation, and diagnostic contract gaps.
2. Reduce preparation and response overhead without weakening determinism.
3. Expand judged corpora and release gates.
4. Improve portability, storage lifecycle, and product integration only after the preceding evidence is stable.

## Completed foundation

- Lexicon and Arcana consolidated into the Grimoire repository while retaining independent applications, state, protocols, and specialist commands.
- Grimoire namespaced Lexicon and Arcana administrative commands.
- Content-addressed prepared source state with deterministic exact and BM25 retrieval.
- Separate documentation indexing with BM25 and optional vectors.
- Lexicon-grounded symbols and Arcana-backed direct relationships, trace, paths, and impact.
- Stable snapshot-qualified handles and exact follow-up inspection.
- Independent exact, source, document, symbol, and relationship limits.
- Automatic aligned preparation across Grimoire, Lexicon, Arcana, and documentation state.
- Persistent investigation sessions with evidence deduplication.
- Managed local embedding runtime commands and Lodestone-backed optional vector state.
- Root build, test, subset installation, deterministic packaging, release checksums, and GitHub release workflow.
- Combined release bundles containing provider binaries, runtime adapters, native library, and canonical Grimoire agent skill.
- Repository-owned retrieval, graph, documentation, and end-to-end agent evaluation.
- Hidden-rubric version 2 agent-quality suite completed across all five planned task classes, with the 15-run Sol/High/Fast comparison and follow-up narrow-task mitigation retained as benchmark evidence.
- Installed release-path warning contract verifies known C-family macro reasons remain typed and future unresolved-reason labels survive Lexicon and Arcana while surfacing as Grimoire compatibility warnings.
- First-use preparation profiling and optimization cut cold Space Rocks Lexicon initialization from 90.64 seconds to 45.00 seconds without changing semantic output; the dominant GDScript path fell from 42.01 seconds to 6.27 seconds in a byte-identical controlled comparison. Discovery now allows two minutes by default so legitimate cold preparation is not discarded as provider failure.
- Retired context-package compiler, query-shape, assembly, curation, and package-fitting paths removed from the active CLI and MCP product.

## Near-term priorities

1. Calibrate compact search defaults, excerpt caps, duplicate-payload references, and degraded-provider response bounds against end-to-end agent outcomes.
2. Expand judged corpora across repositories, languages, sizes, and task categories.
3. Add stable machine-readable diagnostic codes and documented exit classes.
4. Improve installation verification and host-specific MCP setup guidance as supported hosts stabilize.

## Agent discovery quality

- Add benchmark repetitions and confidence intervals for task-shape comparisons.
- Measure time to first required evidence separately from final completion.
- Attribute model cost to Grimoire output, direct source reads, shell search, and repeated evidence.
- Add judged cross-language and generated-contract tasks.
- Add negative-claim cases that require warning, truncation, and provider-coverage handling.
- Improve stopping guidance only when measured agent behavior shows unnecessary discovery calls.
- Prevent source/lexical fallback from expanding unboundedly when Arcana is unavailable.
- Improve canonical-handle transfer so final evidence ranges match inspected ranges exactly.
- Preserve the rule that agents may stop using Grimoire when direct inspection becomes cheaper.

## Prepared-state maintenance

- Add optional repository watching or Warlock-fed change events without making one-shot commands depend on a daemon.
- Add lazy or bounded prepared-state reads for very large repositories.
- Make file eligibility and generated-content policy configurable without weakening permanent state exclusions.
- Calibrate semantic declaration chunking against judged retrieval and downstream-agent token use.

## Retrieval and evidence lanes

- Calibrate document BM25/vector behavior on external rationale-discovery tasks.
- Improve direct relationship seed matching through judged task-shaped cases.
- Add conflict and provenance diagnostics across source, documentation, Lexicon, and Arcana evidence.
- Add external evidence providers only behind concrete, independently testable interfaces.
- Define a stable external provider contract after current integrations settle.
- Keep heterogeneous lanes independently ranked unless judged failures demonstrate a need for another explicit contract.

## Vector-engine work

- Add safe reachability-based immutable-object cleanup.
- Add non-Windows Go dynamic-library loaders and release packaging.
- Benchmark float32 against float16 and int8 encodings.
- Optimize exact-scan kernels only when measurements show material benefit.
- Consider approximate indexing only when exact search is no longer acceptable and exact fallback remains available.
- Evaluate a more efficient ingestion boundary after measuring serialized persistence cost.

## Distribution and compatibility

- Decide whether former Arcana and Lexicon repositories should become automated compatibility mirrors.
- Define canonical module and package import paths before a stable release.
- Add repository-wide contribution guidance.
- Define prepared-index, documentation-vector, Arcana-vector, Lexicon, Arcana, and embedding-runtime migration policy.
- Add managed embedding runtime artifacts for additional platforms.
- Add Warlock lifecycle integration for component discovery and state maintenance while keeping components independently usable.

## Release gates

Establish measured release gates for:

- end-to-end agent correctness and evidence compliance;
- lookup and architecture task efficiency;
- preparation latency and memory;
- deterministic source and document retrieval;
- adapter correctness;
- graph correctness, Lexicon-to-Arcana compatibility, and protocol compatibility;
- native ABI stress;
- installation, skill discovery, and MCP startup.

## Longer-term investigation

- Learned or model-assisted policy only where deterministic rules are insufficient and decisions remain inspectable.
- Repository-scale prioritization and bounded evidence streaming for very large codebases.
- Additional Warlock evidence sources such as Demon Docs or Git-change context when concrete investigations justify them.

The retired context-package and answer-shaped bundle pipeline is not roadmap work. Future context delivery must remain progressive, inspectable, and justified by end-to-end agent outcomes.

Each roadmap item requires an owning seam, verification plan, and documentation update before it becomes current behavior.

## Acceptance criteria

Each item requires a named owner, implementation plan, focused tests or evaluation, documentation impact, and a clear current-behavior owner before it can be marked complete.

## Open decisions

Open decisions include which task classes justify additional discovery automation, how large-repository prioritization should remain inspectable, and whether new evidence providers produce enough measured value to warrant a durable product boundary.

## Related docs

- [System overview](../architecture/system-overview.md)
- [Current limitations](../limits/current-limitations.md)
- [Discovery quality](../development/retrieval-quality.md)
- [Agent benchmark findings](../development/agent-benchmark-findings.md)

## Notes

A roadmap item becomes current only after implementation, protecting verification, and migration into the appropriate current owner.
