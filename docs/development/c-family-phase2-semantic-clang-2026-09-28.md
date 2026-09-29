# C-family Phase 2.4 semantic Clang observations

Parent index: [Development Documentation](INDEX.md)

**Date:** 2026-09-28

**Branch:** `refactor/c-family-clang`

**Starting checkpoint:** `aacc0253a533a11c400ae4f1bd749d9032be64cf` — `Add C-family Clang structural observations`

**Helper protocol:** `1`

**Helper version:** `0.3.0`

## Purpose

Phase 2.4 moves compiler-owned C/C++ relationship and call-resolution evidence onto the staged Clang/LibTooling path without changing the production adapter route.

The staged path is now:

```text
Clang AST/Sema
    |
    | declarations + exact semantic relationships/calls
    v
private semantic observations
    |
    v
Rust C-family materialization/policy
    |
    v
RepositoryModel / facts-v1
```

`CFamilyAdapter::analyze()` still uses the existing Rust/Tree-sitter model. Production ownership moves only at the Phase 2.6 hard cut, after Phase 2.5 rebases pointer/callback/dataflow semantics.

## Clang-owned evidence

The helper now emits compiler evidence for:

- C++ base-class relationships from attributed `CXXRecordDecl` definitions;
- C++ override relationships from `CXXMethodDecl::overridden_methods()`;
- direct calls and selected callees;
- C++ member calls;
- constructors;
- overloaded operators;
- explicit destructor-call form when represented by a call expression;
- unresolved/dependent overload candidate sets where Clang retains them;
- receiver type identity/evidence for member/operator calls;
- virtual-dispatch status from Clang member-expression semantics;
- source argument expressions for the later Phase 2.5 dataflow rebase.

Compiler USRs remain correlation keys only. They never become Lexicon node IDs.

## Overload evidence

For unresolved/ambiguous overload expressions, the helper emits the actual compiler candidate set retained by Clang.

After successful overload resolution, Clang's expression AST retains the selected declaration and whether it came from a multi-candidate overload set, but not the complete viable-candidate set. Phase 2.4 therefore emits:

- the selected target;
- `overload_selected = true` when Clang records that multiple candidates existed;
- `compiler_candidate_count = 1` for the selected emitted target rather than inventing a candidate count that the AST no longer exposes.

No Lexicon-side overload reconstruction is added to fill that gap.

## Rust ownership

Rust remains responsible for:

- mapping compiler entities onto path-owned canonical Lexicon nodes;
- choosing the existing same-file / preferred-definition node when one compiler entity has multiple repository declarations/definitions;
- `calls` versus `possible-calls` relation policy;
- unresolved-reason vocabulary;
- external-target handling;
- virtual-dispatch graph expansion through repository override evidence;
- canonical IDs, ownership, deterministic ordering, and facts-v1 materialization.

The compiler-selected semantic entity is authoritative. Rust's definition preference is an ownership/materialization policy, not a second overload resolver.

## Relationship policy

Resolved repository-local inheritance and override targets become direct `extends` and `overrides` edges.

Compiler relationships whose targets live outside the repository remain explicit unresolved records with `external-target`; missing repository materialization remains `missing-target`.

## Call policy

Rust translates the semantic call observations as follows:

| Compiler evidence | Lexicon result |
| --- | --- |
| resolved static repository target | definite `calls` |
| resolved external target | unresolved `external-target` |
| ambiguous compiler candidate set | `possible-calls` to repository candidates + unresolved `ambiguous-target` |
| dependent or indirect compiler call | unresolved `dynamic-target` |
| virtual member dispatch | `possible-calls` to the selected base target plus known repository overrides + unresolved `dynamic-target` |

Virtual target expansion is intentionally repository-bounded. It does not claim that the repository contains every possible runtime subclass.

## Deliberate deferrals

Phase 2.4 does not move:

- function-pointer target propagation;
- callback/value-flow rebasing;
- passes-to relationships;
- read/write dataflow;
- macro-expanded higher-order call propagation.

Those remain Phase 2.5 so compiler binding evidence is established before graph-specific value flow is rewired.

Phase 2.4 also does not remove Tree-sitter/resolver modules. They remain the one production path until the Phase 2.6 hard cut and are deleted rather than retained as a fallback after cutover.

## Determinism

The helper sorts and deduplicates semantic relationship/call observations before JSON emission.

Header observations produced through multiple translation units therefore do not multiply equivalent relationship/call evidence.

Rust independently deduplicates final facts using the existing facts-v1 record key policy.

## Verification

Repository-owned verification covers:

- semantic protocol decoding;
- compiler-ID to Lexicon-ID materialization;
- same-file/preferred-definition target ownership;
- inheritance and override edge policy;
- definite resolved calls;
- ambiguous compiler candidates;
- virtual-dispatch expansion;
- resolved external targets;
- the existing Phase 2.1 exact oracle to prove production remains unchanged;
- the C-family unit/integration suites;
- the permanent nine-language performance regression gate.

## Native helper build limitation

The current Windows host still lacks the LLVM/Clang development CMake packages required to link the LibTooling helper.

CMake reaches native compiler detection and then fails at `find_package(LLVM REQUIRED CONFIG)` because `LLVMConfig.cmake` is absent. Therefore the C++ helper implementation is statically reviewed and its Rust protocol/materialization path is covered with synthetic compiler observations, but the native Phase 2.4 emitter cannot be linked or executed on this host.

No system packages are installed as part of this phase.

## Gate

Phase 2.4 is complete when:

- Clang relationship/call evidence has a versioned observation contract;
- Rust owns final relationship/call/unresolved policy;
- compiler identities remain non-persistent correlation keys;
- same-file/definition ownership is deterministic;
- production Tree-sitter facts remain unchanged;
- C-family and multilang regression gates pass;
- native helper build limitation is recorded explicitly.

## Next

Phase 2.5 rebases Lexicon-specific pointer, callback, argument/value propagation, and dataflow semantics on top of Clang-bound declaration/call evidence.

Phase 2.6 then performs the production hard cut, deletes the Tree-sitter/compiler-reconstruction path, and leaves Clang as the sole C-family syntax/compiler-semantic authority.
