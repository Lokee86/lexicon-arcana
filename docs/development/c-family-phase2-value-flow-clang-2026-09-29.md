# C-family Phase 2.5 Clang value-flow rebase

Parent index: [Development Documentation](INDEX.md)

**Date:** 2026-09-29

**Branch:** `refactor/c-family-clang`

**Starting checkpoint:** `852d88d` — `Add C-family Clang semantic observations`

**Helper protocol:** `1`

**Helper version:** `0.4.0`

## Purpose

Phase 2.5 rebases Lexicon-specific higher-order and dataflow semantics on compiler-bound Clang observations without changing the production C-family route.

The staged path is now:

```text
Clang AST/Sema
    |
    | declarations + relationships/calls + value-flow evidence
    v
private semantic observations
    |
    v
Rust C-family materialization
    |
    | canonical IDs + graph policy
    v
calls / possible-calls / passes-to / reads / writes
```

`CFamilyAdapter::analyze()` remains on the existing Rust/Tree-sitter model until the Phase 2.6 hard cut.

## Clang-owned value-flow evidence

The helper now emits compiler-bound observations for:

- call arguments with source text plus resolved value and callable identities when Clang supplies them;
- indirect callees bound to the function-pointer variable or parameter being invoked;
- direct function-pointer initializers and assignments;
- designated function-pointer field initializers;
- C++ in-class function-pointer field initializers;
- resolved variable/field read and write targets;
- initialized local-variable writes;
- macro-expansion provenance on compiler-observed calls.

Clang identities remain transient correlation keys. Rust converts them to canonical Lexicon IDs before graph policy runs.

## Rust-owned graph semantics

Rust remains responsible for:

- repository-local canonical identity and source ownership;
- `passes-to` materialization from compiler-bound arguments to canonical parameters;
- fixed-point callback propagation through function-pointer parameters;
- repository-bounded indirect-call target sets;
- `possible-calls` versus definite `calls` policy;
- retaining `dynamic-target` unresolved records for indirect calls even when repository targets are known;
- final `reads` / `writes` edges;
- deterministic facts-v1 emission and deduplication.

The pointer/callback index is graph analysis, not a replacement C/C++ binder: it consumes compiler-bound pointer and argument identities rather than resolving names from syntax text.

## Macro-mediated higher-order flow

The staged path does not reconstruct macro call bodies in Rust.

Clang observes the expanded call AST. Calls whose expression locations originate from macro expansion carry `macro_expanded = true`; Rust records `clang-macro-expansion` evidence on resulting call edges.

This leaves preprocessing and macro expansion under Clang while preserving Lexicon's graph provenance.

## Legacy path status

The existing Tree-sitter modules remain unchanged and production-owned through Phase 2.5:

- `pointer_bindings.rs`;
- `indirect_calls.rs`;
- `dataflow_extract.rs`;
- `dataflow_facts.rs`;
- `macro_facts.rs` and related macro-expansion modules.

They are not fallback infrastructure for the future Clang path. Phase 2.6 deletes or disconnects them when production ownership cuts over.

## Determinism and module boundaries

Compiler observations are normalized and deduplicated before JSON emission.

Rust uses one shared compiler-reference index for calls, relationships, pointer bindings, arguments, and accesses so ownership selection remains consistent across the staged path.

The new value-flow implementation is split into focused modules rather than expanding the AST visitor or semantic call materializer into catch-all components.

## Verification

Phase 2.5 focused coverage proves:

- compiler-bound reads and writes materialize directly to canonical nodes;
- compiler-bound call arguments generate `passes-to`;
- direct function-pointer bindings seed indirect targets;
- callback arguments propagate pointer targets to function-pointer parameters to a fixed point;
- indirect calls emit repository-bounded `possible-calls` plus `dynamic-target`;
- macro-expanded indirect calls preserve both macro-expansion and function-pointer evidence;
- existing Phase 2.3/2.4 structural, identity, relationship, and call-policy tests remain green.

The production Phase 2.1 oracle remains the cutover compatibility boundary and must remain unchanged through Phase 2.5.

## Native helper build limitation

The current Windows host still lacks the LLVM/Clang development CMake packages required to link the LibTooling helper. The C++ observation implementation is therefore covered through Rust protocol/materialization fixtures plus direct API review, while native helper build/runtime acceptance remains unavailable on this host.

No LLVM/Clang system packages are installed as part of this phase.

## Gate

Phase 2.5 is complete when:

- value/callable identities travel through the private observation seam;
- pointer/callback propagation consumes compiler-bound identities rather than syntax name resolution;
- `passes-to`, reads, writes, and indirect-call graph policy remain Rust-owned;
- macro-expanded higher-order calls use Clang expansion evidence;
- production Tree-sitter output remains unchanged;
- focused C-family, exact oracle, documentation, packaging, and performance gates pass.

## Next

Phase 2.6 performs the production hard cut:

- route C-family analysis through the Clang frontend;
- remove Tree-sitter/compiler-reconstruction ownership from the production path;
- delete migration-only and retired C-family parser/resolver code;
- calibrate any authoritative Clang semantic differences against the frozen Phase 2.1 oracle rather than blindly restoring old approximations;
- leave Clang as the sole production syntax/compiler-semantic authority.
