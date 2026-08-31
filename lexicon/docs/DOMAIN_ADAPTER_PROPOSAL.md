# Domain Adapter and Learned Profile Proposal

Parent index: [Lexicon documentation](README.md)

**Status:** Future proposal; not implemented behavior or a compatibility guarantee.

## Purpose

Record the proposed extension of Lexicon beyond programming-language source without presenting it as current behavior or a compatibility guarantee.

## Overview

The proposal keeps one normalized Lexicon analysis boundary while adding broad domain adapters and learned profiles for narrower conventions. It is planning material only.

## Goal

Extend Lexicon beyond programming-language source while preserving one normalized analysis boundary.

The proposed model is:

- broad **domain adapters** own general interpretation for a semantic domain;
- **Profiles** are learned extensions that improve an adapter for narrower conventions;
- profiles do not define a second output format;
- users create profiles from examples and corrections, not by authoring schemas.

## Planned first domain adapters

### Business

One broad Business adapter should automatically handle ordinary business artifacts, including:

- invoices;
- contracts;
- spreadsheets;
- quotes and estimates;
- purchase orders;
- reports;
- forms;
- schedules and tabular business records;
- other structured or semi-structured business documents.

Document-type classifiers and specialized parsers may exist internally, but invoice, spreadsheet and contract handling should not become separate user-facing adapters.

### Construction

The Construction adapter should add genuinely construction-specific interpretation, including:

- construction and engineering terminology;
- drawings, sheets, details, callouts and revisions;
- specifications;
- RFIs, submittals and change documents;
- components, assemblies and materials;
- locations, levels, grids and areas;
- construction shorthand and jargon;
- construction-specific identity and relationship candidates.

## Construction symbol model

Construction symbols should be treated as **semantic concepts and addressable instances**, not only formal identifiers or graphical glyphs.

Examples of useful canonical concepts include:

- contract;
- invoice;
- header;
- lintel;
- beam;
- joist;
- wall;
- footing;
- load;
- span;
- bearing;
- RFI;
- submittal;
- drawing;
- detail;
- specification;
- change order;
- cost code;
- project;
- vendor;
- work item.

The concept may be stable while the instance syntax is not. An invoice ID, project number, change number or drawing tag can follow a different convention for every organization or project.

The adapter therefore should resolve meaning from context rather than assuming globally standardized identifier syntax.

## Profiles

A Profile is an **adapter extension** that teaches an adapter how a narrower context expresses the domain.

Possible scopes include:

- client;
- vendor;
- GC;
- consultant;
- engineer or architect;
- project;
- trade;
- region or jurisdiction;
- software/system;
- recurring document template;
- internal organization convention.

Multiple profiles may be active together.

Example:

`Construction + wood-frame + Client X + Project Y`

Profiles should enrich recognition and resolution before normalized Lexicon records are emitted.

## Required profile UX

Normal profile creation must be short and non-technical:

1. Identify the context.
2. Supply representative examples.
3. Infer conventions automatically.
4. Present a short plain-language summary.
5. Let the user confirm or correct it.
6. Activate the profile.

A user should never need to understand the internal representation to create a useful profile.

Canonical rule:

> **Profiles are learned from examples and corrections, not authored as schemas.**

Corrections made during normal use should be eligible to update the applicable profile.

## Proposed internal profile contract

The internal representation should be standardized enough to support deterministic loading, composition, testing and updates.

It will likely need fields for:

- applicability/context;
- vocabulary and aliases;
- identifier interpretation;
- document/template cues;
- field mappings;
- structural/layout patterns;
- normalization targets;
- relationship hints;
- confidence and precedence;
- provenance and version.

These are generated implementation details, not required user input.

## Composition

Profiles should compose from broad to specific. A likely order is:

`domain core -> trade/region -> organization -> project/template`

More-specific evidence may refine or override broader interpretation. Conflicts should remain explicit and reviewable rather than silently collapsing to certainty.

## Output boundary

The proposal should preserve the existing architectural idea that adapters normalize source analysis into shared Lexicon records.

A Profile may change **how an adapter recognizes or resolves source material**, but it should not introduce a profile-specific downstream schema.

Arcana and Grimoire should continue consuming normalized Lexicon output rather than learning every profile dialect independently.

## Planning tasks

Before implementation:

1. Define the initial Business and Construction canonical kinds/concepts.
2. Identify the minimum shared record additions, if any, required for non-code domains.
3. Define the minimal serialized Profile contract.
4. Define profile loading, applicability and precedence.
5. Design example-to-profile inference.
6. Design correction-to-profile update behavior.
7. Build representative business and construction fixtures.
8. Define acceptance tests for recognition, false bindings, profile conflicts and profile improvement.
9. Prototype the simple profile-creation workflow before exposing advanced manual editing.

Full drawing/object perception is not required for the first useful Construction adapter. Start with high-signal terminology, explicit identifiers, text, tables, schedules and document structures, then expand into visual construction evidence.

## Related docs

- [Lexicon documentation](README.md)
- [Architecture](ARCHITECTURE.md)
- [Current status and limits](STATUS.md)

## Notes

This document remains a future proposal. Current adapter and output guarantees are defined by the linked current-state documentation and versioned contracts.