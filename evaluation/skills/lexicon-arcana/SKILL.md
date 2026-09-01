---
name: lexicon-arcana
description: Use the benchmark's prepared Lexicon facts and Arcana graph directly, without Grimoire.
---

# Lexicon + Arcana benchmark condition

Use only the prepared Lexicon and Arcana component surfaces for optional structured discovery. Grimoire and Codebase Memory are not part of this condition.

The harness provides:

- `LEXICON_BENCH_EXPORT`: a directory of verified Lexicon `<language>.jsonl` exports for the pinned checkout.
- `ARCANA_BENCH_SNAPSHOT`: the current verified Arcana repository-snapshot directory.
- `LEXICON_BENCH_REPO`: the pinned repository checkout.
- `lexicon` and `arcana` on `PATH`.

Normal shell, Git, and direct source-file inspection remain available. Treat component output as discovery evidence; verify material implementation claims in source before the final answer.

## Lexicon

Search the prepared normalized fact export directly. On PowerShell, for example:

```powershell
rg -n -i 'term|OtherTerm' $env:LEXICON_BENCH_EXPORT
```

The JSONL records contain declarations, source spans, relationships, dependencies, and unresolved evidence produced by the enabled language adapters. Use narrow terms first; do not dump complete exports into context.

Useful diagnostics:

```powershell
lexicon status --repo $env:LEXICON_BENCH_REPO
lexicon doctor --repo $env:LEXICON_BENCH_REPO
```

Do not rebuild or rescan during the benchmark. The harness has already prepared the immutable snapshot.

## Arcana

Use the overlay-aware `arcana.query.v1` protocol against `ARCANA_BENCH_SNAPSHOT`. A single request can be piped to the native protocol process:

```powershell
'{"id":1,"op":"search_nodes","query":"background compaction","limit":12}' | arcana protocol --snapshot $env:ARCANA_BENCH_SNAPSHOT
```

Start with `search_nodes` or exact symbol/file resolution, then use returned `node_id` values for graph operations.

Common request shapes:

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

Use relationship filters only when the task requires them. Prefer bounded queries and stop graph exploration when direct source inspection is cheaper.

## Evidence discipline

Lexicon and Arcana identify likely files, symbols, and structural relationships. Final citations must still point to the checked-out repository source or documentation using the benchmark's required `path:line` format. Do not cite generated `.lexicon`, `.arcana`, or export files as implementation authority.
