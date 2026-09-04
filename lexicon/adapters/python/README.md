# Lexicon Python adapter

A self-contained Python package that emits Lexicon facts v1 JSONL using only the Python standard library and `ast`.

## Usage

From the repository root:

```sh
PYTHONPATH=adapters/python python -m lexicon_python \
  --repo /path/to/repository \
  --output /path/to/facts.jsonl
```

`--output -` writes JSONL to stdout. The output directory is created when needed.

The stream begins with the v1 header, followed by canonically sorted nodes, edges, and unresolved records. JSON keys are serialized lexicographically. Validate a generated stream with:

```sh
python tools/validate_jsonl.py facts.jsonl
```

## Node identities

All node IDs are `sha256:` digests of the v1 identity form. Canonical identities include:

- `repository`: repository root basename;
- `directory`: normalized repository-relative directory path;
- `file`: normalized repository-relative Python file path;
- `module`: dotted module name;
- `type`: dotted module and lexical class name;
- `function`: dotted module and lexical function or lambda identity;
- `method`: dotted module, lexical class, and method name;
- `import`: module name plus source occurrence and bound name.

File nodes carry the SHA-256 content identity of the original file bytes. Absolute checkout paths are never included in facts.

## Static call graph

The adapter resolves repository-local calls through a staged binding and flow model. A single proven target emits `calls`. Multiple legitimate static targets emit `possible-calls`. Calls that require external packages, runtime mutation, reflection, or unsupported dynamic behavior remain explicit unresolved records.

Current static resolution covers:

- package-relative imports, module aliases, package re-exports, function-local imports, and nearest-package disambiguation;
- lexical nested functions and classes;
- direct functions, methods, constructors, `cls(...)`, and `super()`;
- local assignments, later reassignment, branch unions, annotated parameters, annotated fields, and factory return flow;
- loop and comprehension element types, including mapping value iteration;
- return-value chaining such as `factory().run()`;
- inheritance, overrides, descendant expansion for annotated base receivers, and C3 method-resolution order;
- higher-order callable parameters propagated from repository call sites;
- callable containers, subscriptions, mapping accessors, bound-method aliases, callable instances, lambdas, constant-name `getattr`, and `functools.partial`;
- lexical closure capture;
- bare repository-local decorators whose returned wrapper is statically recoverable;
- pytest `parametrize` callback values.

The adapter also emits repository, directory, file, module, type, interface/protocol, function, method, and import nodes; structural `contains` and `defines` edges; resolved `imports`, `extends`, and `implements` edges; `overrides` edges to inherited or protocol contract methods; and source spans for emitted relationships.

It implements Lexicon's semantic fact contract v1 for `control-flow`, `error-handling`, `calls`, and `source-spans`. Python `except` handlers emit normalized `error-handler:python` facts and classify explicit handling as `propagate`, `record`, or `recover`. Empty/pass-only handlers therefore feed Pitlord's language-neutral `swallowed-error` rule without requiring Python syntax in Pitlord.

Protocol and other recognized interface contracts are not runtime call targets. An annotated protocol receiver expands only to repository-local concrete implementors: one proven implementation is `calls`, while multiple implementations are `possible-calls`. Exact concrete construction and C3-inherited methods remain definite when the receiver evidence is concrete. Dynamic `getattr` names, reflection, monkey patching, metaclass-generated members, and runtime class mutation remain unresolved.

## Deliberate unresolved boundaries

- Installed-package internals are not imported or inspected. Calls into the standard library and third-party packages remain builtin or external targets.
- Dynamic imports, non-constant reflection, monkey patching, metaclass-generated members, runtime class mutation, and dynamically synthesized callables cannot be proven from syntax alone.
- Star imports and decorator factories with runtime-dependent arguments remain conservative.
- Framework dependency injection is resolved only where ordinary repository-local value flow exposes the target.
- Parse failures still produce file/module facts plus an unresolved parse record, but no declarations from the failed file.

Python files are scanned deterministically while excluding `.git/`, `.worktrees/`, `.workingtrees/`, `.ddocs/`, `.lexicon/`, `.arcana/`, `.grimoire/`, `.pitlord/`, `.cantrip/`, `.homunculus/`, `.incubus/`, `.ritual/`, `.warlock/`, `.next/`, `__pycache__/`, `.pytest_cache/`, `.bundle/`, `node_modules/`, `target/`, build/dist/virtual-environment directories, and vendor directories.

## Dependency semantics

Literal `[project].dependencies` and `[project.optional-dependencies]` entries in `pyproject.toml` emit repository `depends-on` facts. When project dependencies are absent, literal entries in sorted `requirements*.txt` files are used; `requirements-dev*.txt` and `requirements-test*.txt` receive development/test categories, and editable local requirements receive `path: true`. Repository-local Python imports emit module-to-module local dependencies only when the target module is uniquely resolved. Synthetic targets are facts-v1 `module` nodes with `dependency:python:<normalized-target>` identity and `@dependencies/python/...` paths.

Malformed requirement strings, dynamic/VCS/URL entries, dynamic imports, reflection, and package installation are unsupported and are omitted or left to the adapter's existing unresolved classifications. Manifests are parsed as data and never executed.
## Dataflow facts

The adapter emits conservative `reads` and `writes` edges from functions and lambdas to repository-local parameters, locals, class/module values, and resolved attributes. Assignments write, augmented assignments read and write, and initializer, argument, and return expressions contribute reads. Nested scopes and lexical shadowing are preserved. Dynamic attributes, `getattr`/`setattr`, monkey patching, imports, and built-ins are omitted when no sound local target exists.

## Code map

| Concern | Primary implementation | Related tests |
| --- | --- | --- |
| Entry and orchestration | `lexicon_python/__main__.py`, `adapter.py` | `tests/test_adapter.py` |
| Discovery and AST extraction | `discovery.py`, `extraction*.py` | adapter tests |
| Bindings and resolution | `bindings.py`, `resolution.py` | adapter tests |
| Calls and dispatch | `callgraph*.py` | adapter tests |
| Dependencies and dataflow | `dependencies.py`, `extraction_flow.py` | adapter tests |
| Semantic capabilities and error handling | `semantic_facts.py` | `tests/test_semantic_facts.py` |
| Contract and output | `model.py`, `contract.py`, `emission.py` | adapter tests |

Dynamic imports, monkey-patching, and runtime-only dispatch remain conservative unresolved boundaries.
