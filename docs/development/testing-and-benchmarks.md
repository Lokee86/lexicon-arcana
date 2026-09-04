# Testing and benchmarks

Parent index: [Development Documentation](INDEX.md)

## Purpose

Define the active Lexicon + Arcana correctness, documentation, packaging, and repository-agent benchmark workflows while preserving historical Grimoire evidence.

## Overview

Current verification is owned by Lexicon, Arcana, and the shared L+A release tooling. Grimoire runtime tests, MCP tests, discovery tests, and installed-release smokes were retired with the implementation under ADR 0006.

The active repository-agent comparison is normally:

```text
Plain repository tools
vs.
Plain repository tools + Lexicon + Arcana
```

CBM remains available as an explicit comparison condition when a competitive control is useful. Grimoire is not a selectable condition for new runs.

Historical Grimoire benchmark results, grounding reports, audit logs, and adapters remain checked in so earlier conclusions can still be inspected and revalidated against their pinned revisions.

## Root verification

Run the bounded active matrix with:

```bash
python scripts/workflow.py test
```

The root workflow runs:

1. Pitlord policy validation and repository checks;
2. root documentation governance;
3. Lexicon Go tests;
4. Java and Kotlin adapter Go tests;
5. the C# adapter test harness;
6. Arcana Cargo tests.

The workflow defaults to one worker. Increase concurrency deliberately with `--jobs N`.

Packaging/install behavior is covered by:

```bash
python scripts/workflow.py smoke
python scripts/test_workflow.py
```

Those checks verify that active builds and bundles contain Lexicon, Arcana, and Lexicon adapters but no Grimoire executable, MCP surface, installed Grimoire skill, or Lodestone runtime.

## Direct component verification

Lexicon:

```bash
cd lexicon
go test ./...
```

Arcana:

```bash
cargo test --all-targets --locked --manifest-path arcana/Cargo.toml
```

Component-specific adapter and corpus validation remains documented under the owning component trees.

## Documentation verification

Run all three current documentation checks:

```bash
python .standards/docs_policy/check.py --repo .
python .standards/docs_policy/check.py --repo . --config docs-standard.lexicon.json
python .standards/docs_policy/check.py --repo . --config docs-standard.arcana.json
python scripts/check_docs.py
```

No documentation baseline is permitted.

## Repository-agent benchmark

The active runner is:

```bash
python evaluation/run_agent_benchmark.py --help
python evaluation/run_agent_benchmark.py --check
```

Default conditions are:

```text
plain
lexicon-arcana
```

The L+A condition builds only Lexicon, Arcana, and Lexicon adapters. It prepares aligned immutable Lexicon and Arcana state, exposes the frozen `evaluation/skills/lexicon-arcana/SKILL.md`, and keeps normal shell, Git, and direct source inspection available.

CBM can be requested explicitly:

```bash
python evaluation/run_agent_benchmark.py --condition plain --condition lexicon-arcana --condition cbm
```

`grimoire` is deliberately absent from the accepted condition set. New benchmark summaries use `lexicon-arcana.agent-benchmark.v2` and new provenance uses `lexicon-arcana.agent-benchmark.provenance.v1`.

## Retired task handling

The task catalogue retains the historical `grimoire-state-maintenance-ownership` case with `retired: true`. The new-run selector excludes retired tasks and rejects an explicit attempt to run one.

The task remains in the catalogue because historical summaries and grounding reports may still need their original rubric and repository identity during revalidation. Retired tasks skip current-checkout evidence-prefix validation because the implementation they reference has intentionally been deleted from current HEAD.

## Historical Grimoire evidence

The following remain evidence, not active product surfaces:

- saved Plain/CBM/Grimoire benchmark summaries and reports;
- `grimoire.mcp-audit.jsonl` recordings;
- historical Grimoire grounding-handle support in the result validator;
- historical import and revalidation utilities;
- the `grimoire-context` agent-discovery adapter for frozen context-package artifacts;
- old Grimoire knowledge/Arcana evaluation corpora tied to retired implementation revisions.

Do not rewrite or delete those artifacts merely because the product was retired. Their original names are part of the experimental record.

Historical results may be revalidated without rerunning the retired product:

```bash
python evaluation/revalidate_agent_benchmark.py --task <historical-task-id>
```

The revalidator reconstructs the pinned repository revision when needed and understands old Grimoire audit evidence. It does not make Grimoire a runnable current condition.

## Benchmark controls

A fair current comparison uses:

- the same repository revision and task wording;
- the same model/provider and completion criteria;
- equivalent warm/cold state, reported explicitly;
- normal shell, Git, and direct file access in every condition;
- only the named optional analysis surface for each assisted condition;
- all model calls, token usage, preparation cost, and elapsed time;
- exact citation validation against the pinned checkout;
- no hidden prepared answer or free preassembled context package.

Measure answer quality separately from grounding validity and efficiency. Prepared analysis is useful only if it improves the task outcome or reduces discovery cost without degrading correctness.

## Interpretation

Repository graphs and semantic facts are not expected to win every task. Exact identifiers and small local call chains often remain cheaper with direct inspection. Larger or ambiguous ownership, cross-language, dependency, or architecture investigations are the intended cases where L+A may reduce rediscovery.

The Detekt completion-bounded experiment is retained as current evidence that L+A can reduce fresh input and total investigation work while preserving accepted answer quality. It does not establish a universal performance guarantee.

Historical Grimoire results remain useful specifically as evidence for why the umbrella discovery layer was retired.

## Code map

| Verification surface | Primary implementation or artifact | Protecting checks |
| --- | --- | --- |
| Root active matrix | `scripts/workflow.py` | `scripts/test_workflow.py` and component suites |
| Architecture policy | `tools/pitlord/policy.json`, `tools/pitlord/repository.json` | root workflow and standards CI |
| Documentation governance | `scripts/check_docs.py`, `.standards/docs_policy/` | root and component documentation checks |
| Lexicon correctness | `lexicon/` | package, adapter, corpus, and publication tests |
| Arcana correctness | `arcana/` | Cargo storage, snapshot, ingestion, traversal, protocol, and vector tests |
| Current agent benchmark | `evaluation/run_agent_benchmark.py`, `benchmark_runner.py`, `benchmark_component_ablation.py` | preflight/provenance and grounding validation |
| Historical result compatibility | `evaluation/benchmark_grounding.py`, `revalidate_agent_benchmark.py`, `import_agent_benchmark_run.py` | frozen saved reports and pinned checkout reconstruction |

## Related docs

- [Agent benchmark findings](agent-benchmark-findings.md)
- [Release workflow](release-workflow.md)
- [Behavioral contract matrix](behavioral-contract-matrix.md)
- [Architecture verification](architecture-verification.md)
- [ADR 0006](../decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md)

## Notes

Historical benchmark compatibility is intentionally narrower than product compatibility: old evidence remains readable, but no retired Grimoire runtime, installation, MCP, or discovery behavior is protected for future execution.
