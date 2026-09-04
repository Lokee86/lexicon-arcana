# Historical agent discovery benchmark

Historical status: this evaluation-only harness is retained to score recorded Grimoire-era repository investigations and frozen context-package artifacts. It does not represent a current execution mode, and the retired Grimoire runtime is not required or supported for new runs.

Corpus files use [`schema.v1.json`](schema.v1.json). The initial Space Rocks corpus is [`space-rocks.v1.json`](space-rocks.v1.json), pinned to its recorded revision.

Each case defines:

- the expected ownership boundary;
- required source and structural evidence;
- forbidden unsupported conclusions;
- completion criteria;
- known relevant branches.

Scores include correctness, required-evidence recall, input/output and repeated-input tokens, discovery calls, source opens, evidence timing, irrelevant branches, unsupported claims, and repeatability.

## Historical progressive recordings

The example below documents how complete Grimoire `search`, `inspect`, `trace`, and `impact` recordings were scored while that runtime existed. Reproducing it now requires reconstructing the historical source/module revision; it is not part of current L+A verification:

```powershell
go run ./evaluation/agent_discovery/cmd/agent-discovery `
  --cases evaluation/agent_discovery/space-rocks.v1.json `
  --adapter progressive-jsonl --input .\recordings\grimoire.jsonl `
  --output-dir evaluation\results --name grimoire-space-rocks
```

`progressive-jsonl` accepts one event per line with `adapter`, `run_id`, `case_id`, `time_ms`, `kind`, token usage, path/range/symbol, optional branch/relevance, and claims. A line may instead contain a complete `{events:[...]}` transcript.

`raw` accepts generic JSON or JSONL tool records. It maps common open/read tool names, path and line arguments, symbols, token usage, and claims.

## CBM comparison

CBM execution remains external. A CBM exporter can register its transcript adapter with:

```go
agentdiscovery.RegisterAdapter("cbm", adapter)
```

The historical harness embedded no CBM dependency. Historical comparisons remain interpretable only with their recorded repository revision, task, model, completion criteria, and warm/cold state. New repository-agent comparisons use the active Plain versus Lexicon + Arcana harness documented in `docs/development/testing-and-benchmarks.md`.

## Historical adapter

The `grimoire-context` adapter remains only to score frozen historical context-package artifacts. It is not a current execution mode and must be labeled historical in reports.

The runner writes stable JSON and Markdown reports. Multiple records with the same adapter and case are compared for repeatability.
