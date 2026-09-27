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

The notable point as of 2026-09-25 is that both evidence streams currently point in the same direction: **structured repository intelligence is most valuable when it reduces genuine architectural uncertainty, and should get out of the way once direct inspection is cheaper.**

## Limitations

- The initial cases are retrospective.
- There is no standardized Arcana query log for these sessions.
- The agent already had varying amounts of prior Hermes familiarity across cases.
- The tasks were not sampled randomly.
- Public task outcomes do not isolate Arcana's causal contribution.
- Several investigations mixed Arcana with ordinary source search, Git history, tests, and runtime instrumentation.

These limitations are why this artifact is labeled field evidence rather than a benchmark result.