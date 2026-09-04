# Lexicon semantic fact contract v1

This contract defines language-neutral semantic capability and error-handling facts carried inside Lexicon facts v1 streams. It is intentionally narrower than a universal AST: adapters keep syntax-specific interpretation, while consumers operate on normalized semantic facts.

Semantic records use ordinary facts-v1 `protocol` nodes and `contains` edges. Their node IDs therefore remain language-owned Lexicon v1 identities.

## Capability nodes

Each source file for which an adapter can provide semantic facts emits one capability node:

```text
kind: protocol
name: semantic-capabilities:<language>:<capability>,<capability>,...
qualified_name: @semantic/capabilities/<language>/<repository-relative-path>
path: <repository-relative-path>
```

Capabilities are a set serialized in this canonical registry order:

1. `control-flow`
2. `error-handling`
3. `calls`
4. `source-spans`

An adapter may advertise a subset, but it must not advertise a capability it cannot support for that file. Consumers must fail closed when a rule's required capability set is unavailable.

## Error-handler nodes

A syntactic construct that handles an error condition emits:

```text
kind: protocol
name: error-handler:<language>
qualified_name: @semantic/error-handler/<language>/<repository-relative-path>:<adapter-stable-source-location>
path: <repository-relative-path>
span: <the handled branch or clause>
```

The adapter-stable source location is part of the canonical identity and must remain deterministic for identical source bytes. Its exact representation is language-adapter owned.

## Error-action nodes

A handler may contain zero or more normalized error actions. Each distinct action class is emitted at most once per handler:

```text
kind: protocol
name: error-action:<action>
qualified_name: <handler-qualified-name>/<action>:<adapter-stable-source-location>
path: <same path as handler>
span: <evidence span for that action>
```

The action node is connected from its handler with a `contains` edge.

The v1 action registry is:

- `propagate`: the handler explicitly rethrows, returns, rejects, or otherwise propagates the error outcome;
- `record`: the handler explicitly records or reports the error through recognized logging/reporting semantics;
- `recover`: the handler performs an explicit recovery/control-flow effect such as returning a fallback, invoking recovery work, assigning recovery state, or leaving/continuing the enclosing control flow.

`recover` means that the handler is not semantically empty for the purposes of generic error-handling rules. It does not claim that the recovery is correct.

## Consumer semantics

Consumers must treat capabilities as prerequisites rather than inferred parser features. A rule may evaluate only files whose capability nodes satisfy all of its declared requirements.

For `swallowed-error`, a handler is reportable only when:

- the owning file advertises `control-flow`, `error-handling`, `calls`, and `source-spans`;
- the handler has no contained `error-action:propagate`, `error-action:record`, or `error-action:recover` node.

Consumers must not inspect Rust, TypeScript, JavaScript, Python, or other language syntax to reconstruct these facts. Syntax ownership remains in Lexicon adapters.

## Evolution

New capabilities and action classes require an update to this contract and the shared facts validator. Existing names and meanings are stable within semantic contract v1. A future incompatible representation must use a new semantic contract version rather than silently changing v1 meanings.
