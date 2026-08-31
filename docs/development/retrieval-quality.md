# Discovery quality

Parent index: [Development Documentation](INDEX.md)

## Purpose

This document defines how Grimoire discovery quality is evaluated and attributed across independent evidence lanes and end-to-end agent tasks.

## Overview

Quality reporting separates exact source, BM25 source, documentation, symbols, relationships, handle validity, and agent decision quality instead of collapsing them into one undifferentiated score.

Grimoire's active quality target is successful progressive repository investigation, not the quality of a preassembled context package.

## Evaluate lanes independently

A search can fail in distinct ways:

- exact recovery misses a concrete literal;
- source BM25 misses implementation evidence;
- document retrieval misses intent or rationale;
- Lexicon misses or misidentifies a symbol;
- Arcana or Lexicon relationships omit a useful edge;
- a returned handle fails exact inspection;
- the agent chooses an irrelevant branch despite adequate discovery.

Reports should preserve this attribution rather than collapsing every failure into final recall.

## Required measurements

For each task record:

- repository revision and prepared-state identities;
- query and completion criteria;
- required source, document, symbol, and relationship evidence;
- evidence returned by each lane and rank within that lane;
- follow-up inspect, trace, and impact calls;
- irrelevant paths or branches opened;
- input tokens, output tokens, tool calls, and elapsed time;
- unsupported conclusions.

## Lane metrics

Within one lane, ordinary retrieval metrics remain useful:

- recall at k;
- reciprocal rank;
- precision at k;
- exact owner or symbol hit rate;
- relationship edge coverage;
- document freshness and citation correctness.

Do not compare raw scores across lanes. BM25, Lexicon, and Arcana values are provider-local signals.

## End-to-end metrics

The agent-discovery evaluator measures whether the investigation found the required evidence and respected the ownership boundary. Useful aggregate metrics include:

- task completion rate;
- required-evidence recall;
- structural-evidence recall;
- unsupported-conclusion rate;
- median discovery calls;
- median input and output tokens;
- median time to first required evidence;
- median time to complete evidence;
- irrelevant branch count.

## Assisted-agent comparisons

Use identical tasks, revisions, agent models, normal repository tools, skills, and completion criteria. Give each assisted condition exactly one optional discovery system. Do not provide a hidden prepared answer or exclude setup and refresh costs unless equivalent costs are excluded for every system.

Compare at least:

- answer correctness and evidence support;
- owner-file and symbol discovery;
- relationship/path discovery;
- tool-call count;
- source-open count;
- token use;
- latency;
- irrelevant exploration.

A lower call count is not automatically better if required evidence is missing. A higher recall is not automatically better if it floods the agent with irrelevant branches. Report both.

## Corpus design

Cases should be concrete and implementation-checkable. Each case needs:

- a task;
- an ownership boundary;
- required evidence paths and symbols;
- required structural evidence where applicable;
- forbidden unsupported conclusions;
- completion criteria;
- known relevant branches.

Include exact literals, local symbol ownership, cross-file call paths, configuration readers, documentation rationale, mixed source/document questions, cross-language generated contracts, architecture plans, and impact analysis. Report results by task class rather than averaging lookup and architecture work into one undifferentiated score.

## First-use preparation calibration

Preparation latency is measured independently from agent execution because discovery is allowed to refresh aligned Lexicon, Arcana, source, and documentation state before returning evidence. The timing buckets are diagnostic: optimization is accepted only when deterministic output and provider alignment are preserved.

On the Space Rocks calibration repository, the August 2026 first-use pass reduced cold Lexicon initialization from 90.64 seconds to 45.00 seconds. The changes removed redundant repository walks, parallelized independent cold mirror copies, and replaced repeated repository-wide GDScript declaration scans with deterministic indexes. A controlled old/new GDScript comparison fell from 42.01 seconds to 6.27 seconds while producing byte-identical 49,301,400-byte JSONL output with the same SHA-256 digest.

A healthy complete cold preparation after those changes measured 87.25 seconds internally: 47.43 seconds Lexicon, 2.38 seconds Arcana, 28.62 seconds source preparation, 3.51 seconds documentation preparation, and the remainder in inspection, marker, and verification work. Follow-up decomposition showed that the remaining source cost includes the serialized Lexicon export and semantic-span handoff rather than another demonstrated source-index algorithm defect. The normal discovery timeout is therefore two minutes, preserving a cancellation bound while allowing the measured cold path to complete instead of silently degrading to source-only discovery.

These measurements are calibration evidence for this repository and machine, not universal latency guarantees. Future work should target the serialized provider boundary only when profiling shows it remains material on judged workloads.

## Historical package evaluation

The repository contains older retrieval and package-fitting corpora and reports. They remain historical calibration artifacts for the retired context pipeline. They must not be presented as current unified-discovery benchmarks.

## Code map

| Evaluation area | Primary implementation or corpus | Related tests/reports |
| --- | --- | --- |
| Source exact and BM25 retrieval | `internal/retrieve/` | retrieve package tests and benchmarks |
| Documentation retrieval | `internal/knowledge/`, `internal/knowledgeevaluation/` | `evaluation/knowledge/` reports |
| Lexicon symbol evidence | `internal/lexiconfacts/` | provider tests and retrieval-quality cases |
| Arcana structural evidence | `internal/arcanagraph/`, `internal/arcanaevaluation/` | `evaluation/arcana/` reports |
| Small deterministic corpus | `internal/app/testdata/retrieval-quality/` | app evaluation tests |
| End-to-end agent discovery | `evaluation/agent_discovery/`, `evaluation/agent_benchmark_tasks.v2.json` | retained benchmark runs and findings |

Evaluation code measures bounded corpora and tasks. It does not establish universal answer quality or replace correctness tests.

## Related docs

- [Testing and benchmarks](testing-and-benchmarks.md)
- [Agent benchmark findings](agent-benchmark-findings.md)
- [Unified discovery contract](../reference/agent-query.md)
- [Current limitations](../limits/current-limitations.md)

## Notes

Measured results apply only to their recorded corpus, provider state, model, revision, and task conditions.
