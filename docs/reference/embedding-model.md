# Embedding model

Parent index: [Reference](INDEX.md)

Historical status: this page describes the retired Grimoire-managed embedding runtime and is retained only for prior benchmark/design interpretation. It is not an active product contract.

## Purpose

This document defines Grimoire's embedding identity, endpoint contract, managed runtime, request shaping, verification, and operational boundaries.

## Overview

Embeddings are optional supplemental retrieval inputs. Deterministic source, documentation BM25, and graph correctness remain available without the managed embedding process.

Grimoire uses one fixed local embedding contract for indexing and querying.

## Identity

- Model: `Qwen/Qwen3-Embedding-0.6B-GGUF:Q8_0`
- Grimoire identity: `qwen3-embedding-0.6b-q8_0-512d`
- Runtime: pinned `llama.cpp`
- Default endpoint: `http://127.0.0.1:9876/v1`
- Native dimensions: 1,024
- Stored dimensions: first 512 Matryoshka dimensions
- Normalization: L2 inside Grimoire
- Similarity: inner product over normalized vectors
- Document input: raw chunk text
- Query input: repository source-code and documentation retrieval instruction

Prepared vector manifests include the embedding identity and dimensions. Grimoire rejects incompatible state rather than mixing vectors from different contracts.

## Managed setup

On Windows x64:

```bash
grimoire model setup
```

The command downloads the pinned runtime and `Qwen3-Embedding-0.6B-Q8_0.gguf`, verifies fixed SHA-256 digests, and publishes complete artifacts atomically into the user cache. Repeated setup reuses verified files. Use `--cache <path>` to replace the default cache root.

Select a backend explicitly when required:

```bash
grimoire model setup --backend cuda
grimoire model setup --backend vulkan
grimoire model setup --backend cpu
```

`--backend auto` is the default. It selects:

1. CUDA when an NVIDIA driver meeting the managed CUDA 12 runtime requirement is detected.
2. Vulkan when a Vulkan runtime is available.
3. CPU otherwise.

An explicit unsupported or unavailable setup path fails; it is not silently changed to another explicitly unrequested backend. Set `GRIMOIRE_LLAMA_BACKEND` to choose the setup backend without a flag.

Automatic managed setup is currently Windows x64 only. Other platforms must provide a compatible `llama.cpp` executable and set `GRIMOIRE_LLAMA_SERVER` or place it on `PATH`.

## Discovery

Runtime discovery checks, in order:

1. explicit `--runtime` path;
2. `GRIMOIRE_LLAMA_SERVER`;
3. the managed runtime cache; and
4. `llama-server` or `llama` on `PATH`.

Model discovery checks:

1. explicit model path where supported;
2. `GRIMOIRE_EMBEDDING_MODEL`; and
3. the managed model cache.

Inspect discovery without starting the service:

```bash
grimoire model info
```

## Serving

Start the managed detached endpoint:

```bash
grimoire model start
```

The managed supervisor resolves and verifies the requested backend, launches `llama.cpp`, waits for a real embedding probe, records the runtime contract, rotates logs, and restarts a crashed child up to the configured limit. Use `grimoire model status` for health, process IDs, backend verification, context limits, and available NVIDIA telemetry. Use `grimoire model stop` or `grimoire model restart` for lifecycle control.

Start a blocking foreground endpoint when direct console ownership is required:

```bash
grimoire model serve
```

Default service settings:

| Setting | Default |
| --- | ---: |
| Host | `127.0.0.1` |
| Port | `9876` |
| Context size | 8,192 |
| Physical batch size | 2,048 |
| Parallel server slots | 4 |
| GPU layers | all for CUDA/Vulkan; none for CPU |
| Per-slot context | 2,048 |
| Accepted input limit | 1,920 tokens |
| Log rotation | 16 MiB, 3 backups |
| Crash restarts | 5 |

The server runs in embedding mode with last-token pooling. Grimoire performs 512-dimensional truncation and normalization after the response. Requests to the managed endpoint are token-counted before transmission and rejected locally when an individual input exceeds the recorded per-slot limit.

Verify any live OpenAI-compatible endpoint:

```bash
grimoire model probe
```

The probe embeds one instructed query and one raw document, validates the dimensions, and reports their similarity.

## Query plans

- `fast`: split the complete query into non-overlapping 16-token windows, group at most 64 query tokens per request, and run up to two requests concurrently.
- `full`: embed the complete query once.
- `quality`: embed the complete query and all split windows.

`--query-max-tokens 0` retains the complete query. A positive value is an explicit safety limit.

## Operational boundaries

The embedding package owns model identity, runtime setup and discovery, backend selection and verification, detached lifecycle supervision, runtime state, log rotation, context-limit enforcement, request shaping, truncation, normalization, and optional NVIDIA telemetry. It does not own chunking, vector persistence, retrieval ranking, discovery lane assembly, or agent policy.

## Code map

| Concern | Primary implementation | Related tests |
| --- | --- | --- |
| Model identity and endpoint contract | `internal/embedding/spec.go`, `client.go` | `internal/embedding/client_test.go` |
| Query embedding and batching | `internal/embedding/query.go`, `query_batch.go` | query and batch tests |
| Managed runtime configuration and discovery | `internal/embedding/runtime_config.go`, `runtime_candidates.go`, `runtime_state.go` | corresponding runtime tests |
| Process supervision and telemetry | `internal/embedding/runtime_manager.go`, `runtime_supervisor.go`, `runtime_telemetry.go`, platform process files | `internal/embedding/runtime_test.go`, telemetry/log tests |
| Setup and backend verification | `internal/embedding/setup*.go`, `runtime_backend_verify.go` | setup and backend-verification tests |
| CLI integration | `internal/app/model.go`, `model_runtime.go` | `internal/app/model_test.go`, `model_runtime_test.go` |

The embedding process is an external runtime dependency. Deterministic source and graph correctness do not depend on it.

## Related docs

- [Vector store](vector-store.md)
- [Knowledge retrieval](knowledge.md)
- [Indexing](indexing.md)
- [Current limitations](../limits/current-limitations.md)

## Notes

Changing model identity, dimensions, normalization, truncation, or endpoint semantics requires rebuilding incompatible vector state.
