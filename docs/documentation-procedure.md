# Documentation Procedure

Parent index: [Lexicon + Arcana documentation](INDEX.md)

## Purpose

Define the required workflow for shared, Lexicon, Arcana, and historical-retirement documentation changes.

## Overview

Documentation changes accompany implementation changes and are applied to the narrowest canonical owner. Historical Grimoire evidence is preserved when useful, but current behavior must not be documented against deleted Grimoire implementation.

## Procedure

1. Identify whether the responsibility belongs to Lexicon, Arcana, shared release/governance, a downstream consumer, or historical Grimoire evidence.
2. Update exact public reference and implemented architecture owners in the same change.
3. Update component-local documentation rather than relying on a root summary for component behavior.
4. Update `docs/development/documentation-coverage.md` when active ownership, commands, stateful flows, release surfaces, or contracts change.
5. Update `docs/development/behavioral-contract-matrix.md` when an active invariant or focused protecting test changes.
6. Update benchmark/research material only when method, corpus, result, artifact, or interpretation changes; do not generalize measured outcomes into universal claims.
7. Move implemented work out of planning and record unresolved active gaps in limits.
8. Update focused `## Code map` sections in affected current implementation-facing documents when required by the checker.
9. When retiring behavior, update indexes/governance and mark retained old pages historical so they cannot be mistaken for current contracts.
10. Update every affected root or component index and relative link.
11. Run shared, Lexicon, and Arcana documentation checks plus affected implementation tests.
12. Report documentation impact and known gaps explicitly.

## Code maps and historical pages

Current code maps identify implementation, tests, related artifacts, and non-ownership boundaries for their subject. Historical Grimoire pages may contain old implementation paths for evidentiary context; they are not required to track the current source tree after retirement.

Maintainer maps are routing aids only. The root Grimoire maintainer map is historical; component-local Lexicon and Arcana maintainer maps remain active.

## Verification

```bash
python .standards/docs_policy/check.py --repo .
python .standards/docs_policy/check.py --repo . --config docs-standard.lexicon.json
python .standards/docs_policy/check.py --repo . --config docs-standard.arcana.json
python scripts/check_docs.py
python scripts/workflow.py test
```

For pull-request change-impact enforcement, run `--changed-from <base>` against the shared, Lexicon, and Arcana configurations. Baselines are not accepted; every configured tree must report zero findings directly.

## Related docs

- [Documentation policy](documentation-policy.md)
- [Documentation coverage](development/documentation-coverage.md)
- [Behavioral contract matrix](development/behavioral-contract-matrix.md)
- [Testing and benchmarks](development/testing-and-benchmarks.md)

## Notes

Passing one documentation tree does not establish that the other component trees are current or complete. Historical Grimoire documents must remain visibly historical when retained.
