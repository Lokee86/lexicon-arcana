---
name: lexicon-arcana
description: Use Lexicon semantic facts and Arcana graph queries for bounded repository analysis when they reduce source rediscovery.
---

# Lexicon + Arcana

Use Lexicon and Arcana as optional deterministic repository-analysis tools alongside normal shell, Git, search, and direct source inspection.

Use them when the task needs semantic ownership, cross-file or cross-language relationships, dependency impact, call paths, unresolved references, or architecture structure. For an exact literal, named file, or obvious local implementation, direct source search is usually cheaper.

Current source is implementation authority. Lexicon and Arcana are discovery and structural evidence; verify material conclusions against source before answering.

## Prepare current state

Run preparation once before the first structured query in an investigation. Do not repeat it unless repository source changes.

For an initialized repository:

```text
lexicon scan --repo <repo>
arcana sync --lexicon <repo> --state <repo>/.arcana --register
```

If Lexicon has not been initialized and structured analysis is useful:

```text
lexicon init --repo <repo>
arcana sync --lexicon <repo> --state <repo>/.arcana --register
```

`--register` makes later successful Lexicon publications invoke Arcana synchronization through Lexicon's deterministic consumer hook. Explicit `arcana sync` remains valid.

Useful diagnostics:

```text
lexicon status --repo <repo>
lexicon doctor --repo <repo>
```

Do not force rebuilds merely to investigate. Use `lexicon rebuild` only for an actual state/adapter recovery need.

## Lexicon semantic facts

When normalized semantic facts are useful, export the current immutable snapshot to a temporary directory:

```text
lexicon export --repo <repo> --output <temporary-directory>
```

Search the resulting `<language>.jsonl` files narrowly for declarations, source spans, relationships, dependencies, semantic facts, and unresolved evidence. Do not dump complete exports into model context, and do not treat the temporary export as source authority.

Prefer Lexicon for questions such as:

- which declaration owns this behavior;
- where a language-level relationship originates;
- which symbols or files participate in a cross-language boundary;
- what semantic or unresolved evidence an adapter actually emitted.

## Arcana graph

Arcana's machine boundary is `arcana.query.v1`. The protocol opens one immutable repository-snapshot directory.

Resolve the current managed snapshot from `<repo>/.arcana/CURRENT`. It contains a full `sha256:<digest>` Lexicon snapshot ID; the Arcana snapshot directory is:

```text
<repo>/.arcana/snapshots/<digest>
```

For example, in PowerShell:

```powershell
$id = (Get-Content '<repo>/.arcana/CURRENT' -Raw).Trim()
$snapshot = Join-Path '<repo>/.arcana/snapshots' $id.Substring(7)
'{"id":1,"op":"search_nodes","query":"background compaction","limit":12}' |
  arcana protocol --snapshot $snapshot
```

In a POSIX shell:

```sh
id="$(cat '<repo>/.arcana/CURRENT')"
snapshot="<repo>/.arcana/snapshots/${id#sha256:}"
printf '%s\n' '{"id":1,"op":"search_nodes","query":"background compaction","limit":12}' |
  arcana protocol --snapshot "$snapshot"
```

Start from the narrowest useful operation. Common request shapes are:

```json
{"id":1,"op":"search_nodes","query":"text","limit":12}
{"id":2,"op":"resolve_symbol","name":"ExactName","limit":12}
{"id":3,"op":"resolve_file","path":"relative/path.ext","limit":20}
{"id":4,"op":"neighbors","node_id":123,"direction":"incoming","limit":20}
{"id":5,"op":"neighbors","node_id":123,"direction":"outgoing","limit":20}
{"id":6,"op":"impact","node_id":123,"max_depth":3,"limit":40}
{"id":7,"op":"paths","from_node_id":123,"to_node_id":456,"max_depth":5,"limit":20}
{"id":8,"op":"shortest_call_chain","from_node_id":123,"to_node_id":456,"max_depth":8}
{"id":9,"op":"architecture_summary","path_prefix":"src/","limit":20}
{"id":10,"op":"unresolved","path":"relative/path.ext","limit":20}
```

Use `capabilities` when integrating an unfamiliar Arcana binary. Use relationship filters only when the task requires them.

## Investigation discipline

Follow these rules:

1. Ask one concrete repository question at a time.
2. Use Lexicon for semantic facts and Arcana for graph structure; do not make one imitate the other.
3. Prefer bounded graph queries over broad graph dumps.
4. Inspect the identified source directly before making material implementation claims.
5. Use Git/history only when the question is actually historical.
6. Stop structured exploration once the requested conclusion and smallest relevant boundary are supported.
7. Do not broaden into adjacent consequences, hardening ideas, or alternate paths unless an unresolved fact prevents answering the requested question.

There is no Grimoire discovery layer, MCP surface, stable-handle session, or hidden replacement wrapper in this workflow.
