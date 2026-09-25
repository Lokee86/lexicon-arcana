# Hermes field evidence for Arcana

Parent index: [Development Documentation](INDEX.md)

## Purpose

Record observational evidence from real Hermes Agent maintenance and refactoring work where Lexicon + Arcana was available as an optional repository-analysis tool.

This is deliberately separate from the controlled agent benchmarks. The question here is not whether Arcana wins a benchmark condition. It is whether Arcana materially changes discovery, diagnosis, or implementation decisions during ordinary work on a large production repository.

## Research status

Active field log started September 25, 2026.

The initial cases below are retrospective reconstructions from working sessions. Exact per-query telemetry was not retained for every case, so they are evidence of workflow contribution, not controlled causal measurements. Future cases should be recorded prospectively using the capture schema below.

## Working hypothesis

Arcana is most useful when the repository problem is primarily structural:

- ownership is unclear;
- behavior crosses subsystem boundaries;
- multiple execution paths implement similar policy;
- transitive dependencies or blast radius matter;
- a refactor needs a defensible seam;
- static analysis can eliminate candidate paths even when it cannot identify a runtime cause.

It is expected to add little value when the working set is already narrow and a direct source search identifies the relevant implementation cheaply.

## Initial Hermes cases

| Case | Task class | Observed Arcana contribution | Outcome | Interpretation |
| --- | --- | --- | --- | --- |
| [#119195](https://github.com/NousResearch/hermes-agent/issues/119195) / [PR #119238](https://github.com/NousResearch/hermes-agent/pull/119238) | Cross-boundary runtime diagnosis | Connected Desktop pre-agent fallback resolution with the live agent fallback policy and credential-pool state. The investigation ultimately separated three boundaries: accepting a quota-benched fallback, losing the configured primary in a cached Desktop agent, and stale cross-process cooldown state. | A focused fix and regression coverage were produced across the three boundaries. | Positive structural-leverage case. The useful result was not a symbol lookup; it was locating duplicated policy and state ownership across surfaces. |
| [#119664](https://github.com/NousResearch/hermes-agent/issues/119664) | Cross-language writer diagnosis | Traced the boot-time config write far enough to narrow the writer class, then hit a real graph-coverage boundary in the TypeScript/React side. That failure exposed missing TS callable/call relationships in Lexicon/Arcana rather than producing a false confident answer. | The language-analysis gap became actionable adapter work; subsequent TS/TSX work added missing callable/call coverage. | Positive case with a product defect exposed by real use. Useful both for narrowing the investigation and for revealing where the graph stopped being authoritative. |
| [#119003](https://github.com/NousResearch/hermes-agent/issues/119003) / [PR #122050](https://github.com/NousResearch/hermes-agent/pull/122050) | Cross-boundary corruption diagnosis | Static relationship analysis helped eliminate normal claim/reconcile/delete paths as explanations for the malformed Kanban row state. Once the remaining question became “which runtime writer issued the statement?”, static structure had exhausted its authority. | Work moved to default-on SQL write instrumentation instead of extending the static investigation indefinitely. | Positive negative-evidence case. Arcana was useful because it reduced the plausible static write surface and made the transition to runtime instrumentation defensible. |
| [PR #122491](https://github.com/NousResearch/hermes-agent/pull/122491) | Architectural ownership refactor | Repository-graph work was used to inspect Gateway-to-CLI coupling, collapse many import sites into ownership domains, trace consumers outside the obvious Gateway surface, and test proposed migration seams before moving code. | A phased strangler refactor was opened around explicit ownership domains rather than directory-level movement. | Strong architectural-use case. The task itself was relationship discovery and blast-radius analysis. |
| [#118481](https://github.com/NousResearch/hermes-agent/issues/118481) / [PR #119500](https://github.com/NousResearch/hermes-agent/pull/119500) | Persistence-path diagnosis | Helped verify the persistence/call structure around micro-compaction and the mismatch between positional tail semantics and exact durable identity. | The fix changed carried-row classification to exact identity and added regression coverage. | Supporting case. Arcana aided verification, but the defect was narrower than the cases above. |

## Control-like and low-benefit cases

The field log must retain cases where Arcana was unnecessary or unavailable.

| Case | Task shape | Observation |
| --- | --- | --- |
| [#97898](https://github.com/NousResearch/hermes-agent/issues/97898) / [PR #119632](https://github.com/NousResearch/hermes-agent/pull/119632) | Focused Windows terminal guard | The work was diagnosed and implemented while Lexicon/Arcana was unavailable. Direct source inspection was sufficient. |
| [#120558](https://github.com/NousResearch/hermes-agent/issues/120558) / [PR #120680](https://github.com/NousResearch/hermes-agent/pull/120680) | Local provider-id condition | The named-custom-provider defect had a compact working set and concrete code path. Structured graph discovery was not materially necessary. |
| [#120526](https://github.com/NousResearch/hermes-agent/issues/120526) / [PR #120678](https://github.com/NousResearch/hermes-agent/pull/120678) | Local MCP config transformation | The missing interpolation step was localized enough that direct inspection and a regression test were the natural tools. |

These cases matter because the claim under investigation is task-sensitive. “Use Arcana everywhere” is not supported by the observed workflow.

## Current interpretation

The current field evidence is consistent with the benchmark-era task-size inversion:

- narrow, strongly anchored implementation bugs often do not justify graph discovery;
- broad, ambiguous, cross-layer work benefits when Arcana collapses a large structural search space into explicit ownership, dependency, and path questions;
- elimination is a useful result: proving that a suspected path cannot explain a symptom can justify switching to runtime instrumentation;
- a conspicuous graph failure can be valuable when it correctly exposes missing language or relationship coverage instead of silently fabricating structure.

The defensible working claim is therefore:

> Lexicon + Arcana can provide substantial workflow leverage on structurally ambiguous repository tasks, while providing little or no benefit on already-localized implementation work.

This is an observational claim, not yet a causal performance estimate.

## Prospective capture schema

For each substantial Hermes investigation, record the following before the details are forgotten:

| Field | What to record |
| --- | --- |
| Date and task | Issue/PR or a stable task description |
| Repository revision | Commit or branch state used for the investigation |
| Task class | Local lookup, focused trace, cross-boundary diagnosis, impact analysis, architecture/refactor, or other |
| Initial uncertainty | What was not known before repository exploration |
| Arcana use | Operations used and approximate query count |
| Direct inspection | Searches/file reads used before and after Arcana |
| Novel evidence | Symbols, files, relationships, or paths first introduced by Arcana |
| Elimination | Candidate explanations Arcana helped rule out |
| Decision change | Whether graph evidence changed the diagnosis, seam, scope, or next action |
| Boundary/failure | Missing graph coverage, stale facts, excessive context, or no useful contribution |
| Outcome | Fix, PR, instrumentation, refactor plan, no result, or abandonment |
| Verification | Tests, review outcome, production observation, or other follow-up |
| Counterfactual note | Whether direct inspection already had a cheap obvious path |
| Evidence status | Prospective telemetry, retrospective reconstruction, or controlled comparison |

## Collection rules

To reduce cherry-picking:

1. Log substantial Hermes investigations by task threshold, not by whether Arcana looked good.
2. Retain negative, no-benefit, and tool-failure cases.
3. Separate Arcana-discovered evidence from evidence first found by ordinary source inspection.
4. Do not claim time/token savings unless comparable telemetry exists.
5. Do not claim causality from retrospective cases.
6. Link public issues, PRs, commits, and tests whenever available.
7. Record graph coverage failures as product evidence rather than silently excluding the case.
8. Reassess the working hypothesis after a useful sample of prospective tasks rather than promoting individual anecdotes.

## Next analysis

After roughly 20–30 prospectively logged substantial tasks, summarize by task class:

- Arcana used versus not used;
- whether it introduced novel relevant evidence;
- whether it eliminated a candidate path;
- whether it changed an implementation or diagnostic decision;
- direct search/read count where telemetry is available;
- investigation duration and model usage where comparable;
- success, no-benefit, and failure rates.

The goal is not to prove that Arcana is universally better than ordinary developer tools. The useful question is narrower: **which repository tasks gain measurable leverage from deterministic structural context, and which do not?**

## Related docs

- [Agent benchmark findings](agent-benchmark-findings.md)
- [Testing and benchmarks](testing-and-benchmarks.md)
- [Architecture verification](architecture-verification.md)
- [Current limitations](../limits/current-limitations.md)

## Notes

Public Hermes issues and pull requests establish the task and outcome. Arcana contribution in the initial cases is reconstructed from the working sessions in which those tasks were investigated; those sessions did not retain complete standardized query telemetry. Future entries should use the prospective schema above.
