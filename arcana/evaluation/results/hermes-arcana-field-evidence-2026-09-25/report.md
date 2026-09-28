# Hermes Arcana field evidence — 2026-09-25

## Purpose

Record observational evidence from real Hermes Agent maintenance and refactoring work in which Lexicon + Arcana was available as repository-analysis infrastructure.

This is not a controlled benchmark. The initial cases are reconstructed from working sessions after the fact, so they support claims about observed workflow contribution but not causal estimates of time, token, or success-rate improvement.

## Question

Does Arcana materially change repository discovery, diagnosis, or implementation decisions during real work, and on which task classes does that appear to happen?

## Context

The controlled agent benchmarks already suggest a task-size inversion:

- narrow tasks with strong lexical anchors often favor direct source inspection;
- broad, cross-cutting, architecturally ambiguous tasks can benefit when structured discovery collapses the search space.

Hermes Agent is a useful field environment for checking whether that pattern appears during ordinary maintenance rather than benchmark prompts. Its runtime spans Gateway, CLI, Desktop/TUI, agent/provider, persistence, profile, plugin, and configuration surfaces, with several overlapping ownership boundaries.

## Evidence status

**Observational / retrospective.**

Public issues, pull requests, commits, and regression suites establish the underlying task and outcome. Arcana's contribution is reconstructed from the working sessions used to investigate those tasks. Standardized per-query telemetry was not retained for these initial cases.

No time or token savings are claimed from this dataset.

## Positive and boundary cases

| Hermes case | Task class | Observed Lexicon/Arcana contribution | Outcome | Field interpretation |
| --- | --- | --- | --- | --- |
| [Issue #119195](https://github.com/NousResearch/hermes-agent/issues/119195) / [PR #119238](https://github.com/NousResearch/hermes-agent/pull/119238) | Cross-boundary runtime diagnosis | Repository-structure investigation connected Desktop pre-agent fallback resolution with separately owned live-agent fallback policy and credential-pool state. The investigation ultimately separated three boundaries: accepting a quota-benched fallback, losing the configured primary in a cached Desktop agent, and stale cross-process cooldown state. | A focused fix and regressions were produced across all three boundaries. | Strong example of structural leverage: the difficult part was locating duplicated policy/state ownership across surfaces, not finding one named function. |
| [Issue #119664](https://github.com/NousResearch/hermes-agent/issues/119664) | Cross-language writer diagnosis | The investigation narrowed the boot-time config writer class, then hit a genuine graph-coverage boundary on the TypeScript/React side. That failure exposed missing TypeScript callable/call relationships rather than supporting a false conclusion. | Follow-up TypeScript/TSX callable/call coverage work was added in the Lexicon adapter. | Useful both as investigation support and as evidence that real use can expose missing graph semantics. A conspicuous coverage failure is preferable to fabricated connectivity. |
| [Issue #119003](https://github.com/NousResearch/hermes-agent/issues/119003) / [PR #122050](https://github.com/NousResearch/hermes-agent/pull/122050) | Cross-boundary corruption diagnosis | Static relationship analysis helped eliminate ordinary claim/reconcile/delete paths as explanations for malformed Kanban state. Once the remaining question became which runtime process/statement actually performed the write, static structure had reached its authority boundary. | The investigation switched to default-on SQL-write tracing rather than continuing static speculation. | Positive negative-evidence case: eliminating static candidates changed the next diagnostic action even though Arcana could not identify the runtime writer. |
| [PR #122491](https://github.com/NousResearch/hermes-agent/pull/122491) | Architectural ownership refactor | Graph work was used to inspect Gateway-to-CLI coupling, group many import sites into ownership domains, trace consumers outside the obvious directory surface, and test proposed seams before moving implementation. | A phased strangler refactor was organized around explicit subsystem ownership rather than moving code by directory. | Strong architectural-use case: relationship discovery and blast-radius analysis were the task itself. |
| [Issue #118481](https://github.com/NousResearch/hermes-agent/issues/118481) / [PR #119500](https://github.com/NousResearch/hermes-agent/pull/119500) | Persistence-path diagnosis | Arcana supported verification of the persistence/call structure around micro-compaction and the mismatch between positional tail semantics and exact durable identity. | The fix changed carried-row classification to exact identity with focused regressions. | Supporting case rather than a clean Arcana-dependent result; the defect's working set was narrower than the cases above. |

## Low-benefit and control-like cases

The log must retain tasks where Arcana was unnecessary, unavailable, or added little.

| Hermes case | Task shape | Observation |
| --- | --- | --- |
| [Issue #97898](https://github.com/NousResearch/hermes-agent/issues/97898) / [PR #119632](https://github.com/NousResearch/hermes-agent/pull/119632) | Focused Windows terminal guard | Diagnosed and implemented while Lexicon/Arcana was unavailable. Direct source inspection was sufficient. |
| [Issue #120558](https://github.com/NousResearch/hermes-agent/issues/120558) / [PR #120680](https://github.com/NousResearch/hermes-agent/pull/120680) | Local provider-ID condition | Compact working set and concrete code path. Structured graph discovery was not materially necessary. |
| [Issue #120526](https://github.com/NousResearch/hermes-agent/issues/120526) / [PR #120678](https://github.com/NousResearch/hermes-agent/pull/120678) | Local MCP configuration transformation | Missing interpolation behavior was localized enough that direct inspection plus a regression test was the natural path. |
| Early `nous_cli` strangler setup | Small bounded refactor setup | Full structural machinery was not useful for the initial mechanical boundary creation; direct inspection was cheaper until ownership analysis became the bottleneck. |


## Follow-up observations — Hermes ownership refactor, 2026-09-28

This follow-up records a second retrospective review after several additional days of Hermes Gateway, runtime, plugin, profile, and provider-ownership refactoring. It remains observational evidence rather than a controlled comparison. The new observations are supported by the Hermes refactor history, the existing field cases above, and the repository-scale Hermes measurements gathered while Arcana itself was being repaired.

### Refactor-shape observations

| Observation | Hermes evidence | Interpretation |
| --- | --- | --- |
| Arcana shifted the effective unit of refactoring from files to ownership domains. | The Gateway work progressed through explicit ownership extractions for host/service lifecycle, migration CLI, profiles, process identity/containment, subprocess behavior, stdio/resource limits, storage helpers, plugin dispatch, provider identity, provider discovery, provider declarations, and live auth projection. | Large source files were symptoms; the useful question became which subsystem owns a capability and which dependencies cross that boundary. |
| Structural evidence influenced extraction order, not only discovery. | Provider identity was established before provider declarations and live auth projection; runtime/process ownership was extracted before later storage and plugin/provider work. Boundary tests and import guards were repeatedly used to close the old dependency direction before proceeding. | Arcana's value in this class of work is partly prospective: it helps choose a seam and sequence the hard cut before implementation, rather than merely explaining the repository afterward. |
| Distant consumers are a material source of refactor risk. | Gateway/profile/provider work repeatedly required tracing consumers outside the directory being changed, including command, runtime, test, launch/service, and agent-facing surfaces. | Repository-scale relationship and impact inspection is most useful where lexical locality is a poor proxy for architectural dependency. |
| Structural truth does not replace behavioral verification. | Public-facade regressions, invalid imports, packaging omissions, inert monkeypatch/test seams, and runtime-specific defects were found by tests, source/AST inspection, CLI execution, or review rather than by graph structure alone. | Arcana is an architectural evidence source, not a substitute for compatibility tests, packaging checks, runtime instrumentation, or code review. |
| Negative structural evidence can change diagnostic mode. | Earlier corruption work used static elimination to justify SQL-write tracing; the refactor similarly used closed ownership/import boundaries to narrow remaining failures toward behavior, compatibility, or test-contract defects. | A useful Arcana result can be that the suspected architectural path is absent, allowing investigation to move to a different evidence source. |
| The presence of structural evidence appears to reduce the uncertainty cost of hard-cut refactoring. | The refactor increasingly moved authority to a canonical owner and repaired genuine consumers rather than preserving every shallow wrapper or monkeypatch seam. | This is a qualitative workflow observation, not a causal speed claim. Arcana appears to make aggressive ownership cuts easier to reason about because the blast radius can be inspected independently of file layout. |

The strongest updated interpretation is therefore narrower than “Arcana finds code faster” and stronger than simple navigation assistance:

> During broad Hermes refactors, Arcana's main observed contribution is repository-scale architectural situational awareness: representing ownership, dependency direction, cross-layer consumers, and blast radius well enough to affect refactor boundaries and sequencing before code is moved.

### Hermes as a reciprocal Arcana workload

Hermes has also changed Arcana. Real use exposed graph-coverage and implementation problems that synthetic or smaller workloads did not make as obvious:

- the #119664 investigation exposed missing TypeScript/TSX callable/call coverage;
- Hermes scale exposed stale/shared-relationship and incremental-planning problems;
- Rust-port restoration work exposed materialization and ownership-copy costs in Lexicon;
- Arcana ingestion of the Hermes snapshot exposed that repository-fact representation and managed-sync lifetimes, not packed graph topology, were the dominant storage/memory problem.

The same-generation Hermes storage evidence from 2026-09-27 is concrete:

| Measurement | Legacy | Current measured implementation |
| --- | ---: | ---: |
| Nodes | 1,136,365 | 1,136,365 |
| Visible edges | 2,401,702 | 2,401,702 |
| Unresolved references | 697,792 | 697,792 |
| Packed `graph.arcana` | 47,002,416 B | 47,002,416 B |
| Repository metadata / store | 1,167,673,380 B across TSV metadata | 494,199,053 B including `repository.arcana` + manifest binding |
| Total published generation | 1,214,675,796 B | 541,201,469 B |
| Total reduction | — | 673,474,327 B / 55.44% |

The byte-identical packed graph is important: a roughly 1.14-million-node / 2.40-million-edge Hermes graph occupies about 47 MB. The major storage pathology was outside the graph representation.

Runtime memory remains a separate limitation. On the 2026-09-27 Hermes measurements:

- rebuild: **156.946 s**, **1,592,414,208 B peak process-tree RSS**;
- managed overlay: **201.966 s**, **2,219,360,256 B peak process-tree RSS**;
- a second graph-neutral TypeScript parity case measured **120.623 s / 2,001,457,152 B** for overlay versus **96.947 s / 1,606,561,792 B** for a clean rebuild.

These measurements show that storage restoration and semantic parity passed, while managed-sync lifetime/memory work remained incomplete. Hermes therefore functions not only as a consumer of Arcana but as a repository-scale integration workload that has directly exposed Arcana/Lexicon design defects.

### Operational-friction observation

A 2026-09-28 attempt to query the Hermes Arcana state from the integrated Lexicon/Arcana branch also exposed an operational boundary. Hermes's checked-out `.arcana/CURRENT` still referenced the older TSV/v1 generation for Lexicon snapshot `a73b0627...`; the current integrated Arcana protocol rejected that stale repository manifest as malformed for the newer format.

This is not evidence of semantic graph corruption: the referenced generation is the preserved legacy generation used in the storage comparison. It is evidence that analysis infrastructure loses practical value when generated state and the querying implementation drift apart. For refactor-time use, refresh/version compatibility needs to be cheap and conspicuous enough that Arcana remains operationally close to invisible.

## Current interpretation

The Hermes field cases are consistent with, but do not independently prove, the benchmark-era task-size inversion.

The current defensible claim is:

> Lexicon + Arcana appears to provide workflow leverage when repository uncertainty is structural — unclear ownership, cross-layer execution paths, duplicated policy, transitive dependencies, or blast-radius questions — while providing little or no benefit once the working set is already narrow and directly searchable.

Three additional observations matter:

1. **Elimination is useful output.** Showing that a suspected static path cannot explain a symptom can justify changing diagnostic mode, as in #119003.
2. **Coverage failures are product evidence.** The #119664 investigation exposed a TypeScript graph gap through actual use, providing a concrete adapter improvement target.
3. **Selective use remains correct.** The control-like tasks support the existing benchmark conclusion that forcing structured discovery onto cheap local lookups can add overhead without improving the answer.

This dataset does not establish causal speedup, token reduction, or higher task success. Those require prospective telemetry or matched controls.

## Prospective capture schema

Future substantial Hermes tasks should be logged when they start, not only when Arcana performs well.

| Field | Record |
| --- | --- |
| Date and task | Issue/PR, task description, and repository revision |
| Task class | Local lookup, focused trace, cross-boundary diagnosis, impact analysis, architecture/refactor, other |
| Initial uncertainty | What was unknown before repository exploration |
| Arcana use | Operations used and approximate query count |
| Direct inspection | Search/read operations before and after Arcana |
| Novel evidence | Symbols, files, paths, or relationships first introduced by Arcana |
| Eliminated candidates | Explanations or paths ruled out with graph evidence |
| Decision change | Whether evidence changed diagnosis, seam, scope, or next action |
| Failure/boundary | Missing graph coverage, stale facts, excessive context, or no useful contribution |
| Outcome | Fix, PR, instrumentation, refactor plan, no result, abandonment |
| Verification | Tests, review outcome, production observation, or other follow-up |
| Counterfactual note | Whether direct inspection already exposed a cheap obvious route |
| Evidence status | Prospective telemetry, retrospective reconstruction, controlled comparison |

## Collection rules

1. Log substantial investigations by a task threshold, not by whether Arcana helped.
2. Keep negative, no-benefit, and tool-failure cases.
3. Distinguish evidence first found by Arcana from evidence first found by ordinary inspection.
4. Do not infer time/token savings without comparable telemetry.
5. Do not claim causality from retrospective reconstruction.
6. Link public issues, PRs, commits, and tests whenever available.
7. Treat missing graph coverage as product evidence rather than excluding the case.
8. Reassess the hypothesis after enough prospective cases accumulate.

## Planned analysis

After approximately 20–30 prospectively logged substantial tasks, summarize by task class:

- Arcana use versus non-use;
- novel relevant evidence introduced;
- candidate paths eliminated;
- diagnosis or implementation decisions changed;
- search/read counts where telemetry exists;
- elapsed/model usage where comparable;
- successful, no-benefit, and failed investigations.

The target question is not whether Arcana is universally superior to ordinary developer tools. It is which repository tasks gain measurable leverage from deterministic structural context and which do not.

## Relationship to controlled benchmarks

This field log is complementary to, not a replacement for, the controlled benchmark suite.

The controlled work measures matched conditions with explicit telemetry and hidden rubrics. The Hermes evidence measures ecological validity: whether the same task-sensitive behavior appears while solving unplanned production-repository problems.

The notable point through the 2026-09-28 follow-up is that both evidence streams currently point in the same direction: **structured repository intelligence is most valuable when it reduces genuine architectural uncertainty, and should get out of the way once direct inspection is cheaper.**

## Limitations

- The initial cases are retrospective.
- There is no standardized Arcana query log for these sessions.
- The agent already had varying amounts of prior Hermes familiarity across cases.
- The tasks were not sampled randomly.
- Public task outcomes do not isolate Arcana's causal contribution.
- Several investigations mixed Arcana with ordinary source search, Git history, tests, and runtime instrumentation.

These limitations are why this artifact is labeled field evidence rather than a benchmark result.