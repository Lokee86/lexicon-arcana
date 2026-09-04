# Architecture verification

Parent index: [Development](INDEX.md)

## Purpose

Define the executable policy and focused test gates that protect Lexicon + Arcana ownership, component independence, active release boundaries, and documentation contracts.

## Overview

Pitlord owns repository-level architectural invariants that are reliable to detect statically. Focused Go, Rust, Python, and workflow tests remain the executable proof for detailed runtime behavior.

Grimoire's former runtime, MCP lifecycle, repository state, and Lodestone integration are retired and are no longer protected as active compatibility surfaces.

## Pitlord gate

Run:

```bash
pitlord validate --policy tools/pitlord/policy.json
pitlord check --repo . --policy tools/pitlord/policy.json
```

The current policy protects:

- ADR 0006 and the active retirement decision;
- independently buildable Lexicon and Arcana roots;
- Lexicon independence from retired Grimoire implementation packages;
- bounded Lexicon external-consumer execution;
- explicit root build/test/release ownership;
- absence of Grimoire from active build, install, and release surfaces;
- generated-state ignore ownership.

## Workflow integration

`python scripts/workflow.py test` runs Pitlord before documentation and component tests. Pushes and pull requests run the same policy in `.github/workflows/documentation-standard.yml`.

The root release workflow then builds only Lexicon + Arcana and verifies Arcana's required protocol capabilities before packaging.

## Review requirements

Changes to component ownership, module identities, public protocols, persisted formats, process boundaries, or source-of-truth rules require:

1. an update to the canonical architecture or contract owner;
2. an ADR when the decision is consequential, surprising, cross-cutting, or difficult to reverse;
3. focused tests and an updated Pitlord rule when the invariant is mechanically enforceable;
4. migration and compatibility notes when existing state or consumers are affected.

A passing Pitlord check does not waive those review requirements.

## Code map

| Verification responsibility | Implementation | Related tests or gates |
| --- | --- | --- |
| Canonical policy composition | `tools/pitlord/policy.json` | `pitlord validate` in root workflow and CI |
| Repository architecture rules | `tools/pitlord/repository.json` | `pitlord check` in root workflow and CI |
| Shared release workflow | `scripts/workflow.py` | `scripts/test_workflow.py` |
| Lexicon consumer lifecycle | `lexicon/internal/consumer/` | `lexicon/internal/consumer/runner_test.go` |
| Lexicon semantics/publication | `lexicon/` | Lexicon package and adapter tests |
| Arcana graph/protocol | `arcana/` | Arcana Cargo tests and root protocol capability verification |

## Related docs

- [Component architecture](../architecture/components.md)
- [Behavioral contract matrix](behavioral-contract-matrix.md)
- [Release workflow](release-workflow.md)
- [Architecture decisions](../decisions/INDEX.md)

## Notes

New Pitlord rules should protect a named durable invariant. Do not turn architecture policy into a style checker, encode historical Grimoire behavior as a current contract, or speculate about future package structures.
