# ADR 0001: Monorepo with independent components

Parent index: [Architecture decisions](INDEX.md)

## Purpose

Record why Grimoire, Lexicon, and Arcana are co-located while retaining independent ownership, state, executable, test, and release boundaries.

## Overview

The monorepo is a coordination and packaging boundary, not permission to merge component responsibilities or private state.

## Status

Partially superseded by [ADR 0006](0006-retire-grimoire-lead-with-lexicon-arcana.md). Component independence and the Lexicon/Arcana ownership boundaries remain valid; Grimoire-as-normal-entry-point and three-product packaging do not.

## Context

Grimoire presents one repository-discovery product over language analysis, graph analysis, and product-level retrieval. The implementation is co-located so cross-component contracts, packaging, and verification can evolve together, but Grimoire, Lexicon, and Arcana have different domains, runtimes, state formats, and specialist users.

Canonical current architecture: [Component architecture](../architecture/components.md) and [Analysis stack](../architecture/analysis-stack.md).

## Decision

Keep Grimoire, Lexicon, and Arcana in one source repository while preserving them as independently usable components:

- each has its own executable and direct command surface;
- each owns its implementation domain, state, tests, and focused documentation;
- the root workflow may build, test, package, and install them together;
- Grimoire is the normal discovery entry point and exposes thin `lexicon` and `arcana` namespaces without absorbing component behavior.

Source co-location does not imply shared runtime state or merged ownership.

## Ownership and dependency consequences

- Grimoire owns unified source/document discovery, stable handles, state coordination, sessions, and public CLI/MCP contracts.
- Lexicon owns language adapters, normalized facts, immutable analysis objects, and snapshots. It does not depend on Grimoire or Arcana.
- Arcana owns Lexicon snapshot ingestion, graph storage, traversal, impact, paths, overlays, compaction, and graph protocol behavior.
- Dependency direction is repository source → Lexicon snapshot → Arcana graph, while Grimoire consumes aligned source, document, Lexicon, and Arcana state.
- Cross-component convenience must remain thin and must not move parsing or graph policy into Grimoire.

## State, lifecycle, and failure consequences

- `.grimoire/`, `.lexicon/`, and `.arcana/` remain separately versioned and published.
- No component mutates another component's private state directly; integration uses immutable exports, manifests, and explicit protocols.
- Lexicon and Arcana can run without Grimoire, and Grimoire can retain exact, source, and document discovery when structural providers are unavailable.
- Component build or runtime failures must remain attributable to the owning process and must not corrupt unrelated component state.

## Alternatives considered

- Merge all behavior into one executable and state format. Rejected because it erases ownership, independent use, and failure isolation.
- Keep the components only in separate repositories. Rejected as the sole arrangement because it makes coordinated contracts, releases, and end-to-end verification harder.
- Share mutable state between components. Rejected because it couples formats and publication lifecycles.

## Compatibility and migration impact

The consolidated repository and combined bundle retain separate `grimoire`, `lexicon`, and `arcana` binaries. Existing specialist workflows may invoke Lexicon or Arcana directly; normal product workflows may use Grimoire's namespaced forwarding. Moving code into the monorepo does not authorize state-format unification or silent command reinterpretation.

## Verification

This decision is protected by:

- root command-forwarding and repository-state tests;
- Lexicon application, adapter, object, and snapshot tests;
- Arcana CLI, ingestion, storage, snapshot, and protocol tests;
- combined build, installation, packaging, and release smoke tests.

See the [Behavioral contract matrix](../development/behavioral-contract-matrix.md).

## Risks and debt

- Co-location can invite accidental ownership leakage or direct internal dependencies.
- Coordinated releases can obscure that component formats and commands still have independent compatibility concerns.
- Thin forwarding requires provider discovery and version-skew diagnostics.

## Superseding conditions

Supersede this ADR only if the product intentionally removes independent component use, changes the dependency direction, merges state ownership, or adopts a different repository/release model. Such a change must define replacement ownership, migration, degradation, and compatibility contracts before implementation.

## Related docs

- [Component architecture](../architecture/components.md)
- [Analysis stack](../architecture/analysis-stack.md)
- [Operations and trust boundaries](../architecture/operations-and-trust.md)

## Notes

Canonical architecture documents define current mechanics; this ADR defines the durable rationale and tradeoffs.
