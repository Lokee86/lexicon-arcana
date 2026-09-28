# ADR 0007: Authoritative frontends emit semantic observations

Parent index: [Architecture decisions](INDEX.md)

## Purpose

Define the durable boundary between language-authoritative compiler frontends and Lexicon adapters before the remaining language re-port begins.

## Status

Accepted.

## Context

The production Go adapter already uses the intended runtime direction:

```text
Go frontend/helper -> private protocol -> Rust LanguageAdapter -> Analysis
```

However, its current private protocol mixes three kinds of information:

- compiler/frontend evidence, such as declarations, source spans, resolved symbols, interface relationships, SSA/VTA targets, reads/writes, and diagnostics;
- Lexicon policy, such as `definite` versus `possible` calls, `internal` versus `external` target classes, and final unresolved reasons;
- materialization details, such as Lexicon-shaped identity strings and relationship names that Rust consumes almost directly.

Copying that protocol into Clang, K2, Godot, rustc, CPython, Prism, and javac integrations would create a second facts contract. Replacing it with one universal compiler IR would instead force materially different language frontends into an abstraction Lexicon does not need.

The authoritative-frontend migration therefore needs a stable ownership rule before any second frontend is integrated.

## Decision

Authoritative frontends emit **language-specific semantic observations**. Rust `LanguageAdapter` implementations translate those observations into Lexicon semantics and facts.

The canonical direction is:

```text
repository / project
        |
        v
authoritative language frontend
        |
        v
language-owned observation protocol
        |
        v
Rust LanguageAdapter
        |
        +-- canonical identity policy
        +-- repository/source ownership
        +-- certainty and unresolved policy
        +-- Lexicon-specific semantic extensions
        |
        v
Analysis / facts-v1
```

There is no universal AST, universal compiler model, or cross-language observation enum.

A private frontend protocol may use helper-local semantic keys to correlate observations, but those keys are not Lexicon node IDs and are not persisted as canonical identities. The observation must carry enough semantic description for the Rust adapter to construct the canonical Lexicon identity.

The shared cross-process layer owns execution mechanics only: executable discovery, process lifecycle, framing, bounded I/O, stderr capture, typed decoding, protocol-version checks, and process-boundary measurements.

## Go reference observation vocabulary

The Go protocol is the first implementation of this boundary. Its next protocol version uses the following conceptual vocabulary.

### Symbol reference

A symbol reference identifies compiler evidence without defining a Lexicon node ID.

It carries:

- a helper-local semantic key used only to correlate observations in one response;
- frontend kind;
- language-level name;
- package/namespace information where known;
- receiver/container information where relevant;
- repository-relative declaration owner and span when the symbol is source-backed.

Rust decides whether that evidence represents a repository-local, external, standard-library, generated, or synthetic Lexicon target and constructs the canonical identity.

### Declaration observation

Carries:

- symbol reference;
- declaration form;
- repository-relative owner;
- source span;
- typed container/package/import information needed to reconstruct semantic ownership.

The current unstructured declaration `metadata` map is not the target protocol. Required semantic fields become typed observation fields.

### Relationship observation

Carries compiler-proven relationship evidence such as:

- implements;
- embeds/extends;
- overrides.

Source and target are symbol references or helper-local symbol keys with sufficient target description.

Rust chooses the final Lexicon relationship materialization.

Closure capture evidence is represented as its own capture observation rather than overloading a generic relationship record with `target_name` and `capture_index`.

### Callsite observation

One callsite observation describes compiler evidence for one source callsite. It carries:

- caller semantic key;
- repository-relative owner and span;
- source expression text or an equivalent stable description;
- call form;
- static binding evidence;
- compiler/SSA/VTA runtime target evidence;
- optional lookup hints when the frontend cannot resolve a target.

The call form may distinguish compiler facts such as direct function call, interface dispatch, function-value/dynamic dispatch, builtin invocation, and type conversion.

Static binding evidence distinguishes:

- one resolved symbol;
- compiler-reported ambiguous candidates;
- no resolved symbol;
- an unsupported expression form.

Runtime target evidence is a deterministic set of concrete targets discovered by compiler analysis such as SSA/VTA.

The helper does **not** emit a final Lexicon `calls`, `possible-calls`, `converts-to`, or unresolved decision. Rust derives those from the evidence.

For example:

- one statically justified callable target can become `calls`;
- several defensible runtime targets can become `possible-calls`;
- a type-conversion call form can become `converts-to`;
- an ambiguous static binding can become Lexicon `ambiguous-target`;
- unresolved dynamic dispatch with no concrete target can become `dynamic-target`;
- unsupported syntax shape can become `unsupported-form`.

Those mappings are Lexicon policy, not frontend protocol vocabulary.

### Dataflow observation

Carries compiler-derived read/write evidence:

- containing callable semantic key;
- referenced symbol description;
- read or write access;
- repository-relative owner;
- source span.

`read` and `write` are frontend evidence. Rust maps them to Lexicon `reads` and `writes` relationships.

### Capture observation

Carries:

- closure semantic key;
- captured symbol reference when known;
- captured name;
- deterministic capture position/index when the frontend exposes it;
- source owner/span evidence.

Rust owns any synthetic capture identity and the final `references` relationship.

### Diagnostic observation

Carries frontend/toolchain diagnostics:

- severity;
- frontend code;
- message;
- optional repository-relative owner/span.

Diagnostics do not directly create Lexicon facts unless a language adapter explicitly defines a Lexicon semantic policy for them.

## Current Go protocol ownership cut

The current protocol is migrated according to this table:

| Current field or concept | Final owner / action |
| --- | --- |
| declaration `identity` | split into helper-local semantic key plus semantic description; Rust constructs canonical Lexicon identity |
| declaration `kind`, `name`, `owner`, `span` | retain as frontend evidence |
| declaration `metadata` | replace with typed semantic fields |
| relationship `implements`, `extends`, `overrides` | retain as compiler evidence; Rust materializes Lexicon relationships |
| relationship `references` plus capture fields | replace with dedicated capture observation |
| call `kind=definite/possible/conversion` | delete as frontend policy; Rust derives final relationship |
| call `class=internal/external` | delete; Rust classifies repository ownership from semantic target evidence |
| call `class=builtin/conversion/interface/dynamic` | represent only as compiler call-form/dispatch evidence where applicable |
| target `identity/class/name/namespace/container` | replace with structured symbol reference/target evidence |
| unresolved `relation` | delete; the observation already describes a callsite |
| unresolved `reason` | delete; Rust derives Lexicon unresolved reason from binding/dispatch evidence |
| unresolved expression and candidate hints | retain as frontend evidence |
| dataflow read/write | retain as frontend evidence |
| diagnostic severity/code/message/span | retain as frontend evidence |
| final SHA-256 node IDs and facts-v1 records | Rust/Lexicon only |

## Ownership consequences

The authoritative frontend owns language truth it can directly establish:

- parsing and grammar;
- binding and symbol resolution;
- types and receiver types;
- overload/dispatch candidates;
- inheritance/interface/override evidence;
- compiler/project/module semantics;
- compiler diagnostics;
- frontend-native control/dataflow evidence where available.

Lexicon owns:

- canonical node identities;
- facts-v1 construction;
- source/repository ownership policy;
- local/external/synthetic representation;
- definite/possible/unresolved policy;
- final unresolved reasons;
- deterministic fact ordering;
- incremental replacement semantics;
- repository/dependency evidence outside compiler ownership;
- graph-specific inference not supplied directly by the compiler.

## Alternatives considered

### Frontends emit facts-v1 or a facts-shaped private protocol

Rejected. It moves Lexicon identity, certainty, ownership, and graph policy into every language runtime and creates multiple semantic owners.

### One universal compiler/AST intermediate representation

Rejected. Clang, K2, Godot, rustc, CPython, Prism, javac, Roslyn, Go tooling, and the TypeScript Compiler API expose different useful semantic models. A universal schema would either become a second compiler framework or collapse to the lowest common denominator.

### Keep the current Go protocol as the reference unchanged

Rejected. The current protocol already contains final Lexicon call certainty, target classification, unresolved reasons, and identity shapes. Treating those as frontend concepts would reproduce the ownership leak in every later adapter.

## Migration and compatibility

The Go private helper protocol is internal and will be hard-cut to the new observation shape. Producer and consumer move together.

There is no old/new dual reader, translation layer, compatibility alias, or fallback protocol.

Existing external compatibility boundaries remain facts-v1, canonical identities where semantics remain equivalent, snapshot/publication behavior, deterministic output, and downstream consumers.

## Verification

The boundary is considered correctly implemented when:

- the shared frontend runner contains no language semantics;
- Go helper output contains compiler evidence rather than final Lexicon facts policy;
- Rust owns canonical identity and final relationship/unresolved decisions;
- the Go semantic oracle and deterministic-output tests still pass after adjudicating any old-adapter mistakes;
- no universal AST/compiler-observation framework is introduced;
- the second implementation, Clang-backed C/C++, can use the execution seam without adopting Go-specific semantic types.

## Related docs

- [Authoritative Frontend Re-port](../planning/authoritative-frontend-re-port.md)
- [Adapter authoring guide](../../lexicon/docs/ADAPTER_AUTHORING.md)
- [facts-v1](../../lexicon/spec/facts-v1.md)

## Notes

Standardize how authoritative frontend evidence enters Lexicon. Do not standardize the compilers themselves.
