"""Bounded process-parallel local extraction for the Python adapter."""

from __future__ import annotations

from concurrent.futures import ProcessPoolExecutor
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Iterable

from .contract import content_id
from .discovery import _name_from_dotted, _posix_relative, inventory, load_context
from .extraction import DeclarationVisitor
from .model import Facts, FileContext, RepositorySnapshot
from .semantic_facts import emit_semantic_facts


@dataclass
class ExtractionFragment:
    index: int
    contexts: list[FileContext]
    facts: Facts


@dataclass(frozen=True)
class FileRequest:
    shard_index: int
    file_index: int
    root: str
    repository: str
    relative_path: str


def extract_repository(
    repo: Path,
    workers: int = 1,
    shards: int = 1,
    merge_fan_in: int = 2,
) -> tuple[RepositorySnapshot, Facts]:
    root, repository, directories, paths = inventory(repo)
    partitions = _partition_paths(paths, max(1, shards))
    if not partitions:
        facts = Facts(repository=repository)
        snapshot = RepositorySnapshot(root, repository, directories, [])
        _add_repository_structure(facts, snapshot)
        return snapshot, facts

    requests = _file_requests(root, repository, partitions)
    shard_fragments = [
        ExtractionFragment(index=index, contexts=[], facts=Facts(repository=repository))
        for index in range(len(partitions))
    ]

    worker_count = max(1, min(workers, len(requests)))
    if worker_count == 1:
        results: Iterable[tuple[int, int, FileContext, Facts]] = (
            _extract_file(request) for request in requests
        )
        _merge_file_results(shard_fragments, results)
    else:
        # Keep the IPC unit small on Windows. Returning hundreds of ASTs in one
        # shard result makes pickle cost dominate the actual analysis.
        with ProcessPoolExecutor(max_workers=worker_count) as executor:
            results = executor.map(_extract_file, requests, chunksize=1)
            _merge_file_results(shard_fragments, results)

    merged = _reduce_fragments(shard_fragments, merge_fan_in)
    snapshot = RepositorySnapshot(root, repository, directories, merged.contexts)
    _add_repository_structure(merged.facts, snapshot)
    return snapshot, merged.facts


def _file_requests(
    root: Path,
    repository: str,
    partitions: list[list[Path]],
) -> list[FileRequest]:
    requests: list[FileRequest] = []
    file_index = 0
    for shard_index, partition in enumerate(partitions):
        for path in partition:
            requests.append(
                FileRequest(
                    shard_index=shard_index,
                    file_index=file_index,
                    root=str(root),
                    repository=repository,
                    relative_path=path.relative_to(root).as_posix(),
                )
            )
            file_index += 1
    return requests


def _extract_file(request: FileRequest) -> tuple[int, int, FileContext, Facts]:
    root = Path(request.root)
    context = load_context(
        root,
        request.repository,
        root / Path(request.relative_path),
    )
    facts = Facts(repository=request.repository)
    _prepare_context(facts, context)
    if context.tree is not None:
        DeclarationVisitor(facts, context).visit(context.tree)
        # Semantic capability/error-flow facts are file-local. Emit them in
        # the same worker so cold scans do not walk every AST again serially.
        emit_semantic_facts(facts, [context])
    return request.shard_index, request.file_index, context, facts


def _merge_file_results(
    shards: list[ExtractionFragment],
    results: Iterable[tuple[int, int, FileContext, Facts]],
) -> None:
    # ProcessPoolExecutor.map preserves request order even though workers finish
    # out of order, so collision behavior stays identical to the sorted serial
    # scan.
    for shard_index, _, context, facts in results:
        shard = shards[shard_index]
        shard.contexts.append(context)
        _merge_facts(shard.facts, facts)


def _prepare_context(facts: Facts, context: FileContext) -> None:
    relative = context.relative_path
    context.file_id = facts.add_node(
        "file",
        context.path.name,
        relative,
        relative,
        identity=relative,
        file_content_id=content_id(context.data),
    )
    context.module_id = facts.add_node(
        "module",
        _name_from_dotted(context.module_name),
        relative,
        context.module_name,
        identity=context.module_name,
    )
    facts.modules[context.module_name] = context.module_id
    facts.add_edge(context.file_id, context.module_id, "contains")
    if context.parse_error:
        facts.add_unresolved(
            context.module_id,
            "parses",
            relative,
            "unsupported-form",
            candidate_name=context.parse_error,
        )


def _add_repository_structure(facts: Facts, snapshot: RepositorySnapshot) -> None:
    repository_id = facts.add_node(
        "repository",
        snapshot.repository,
        ".",
        snapshot.repository,
        identity=snapshot.repository,
    )
    directory_ids: dict[str, str] = {".": repository_id}
    for directory in snapshot.directories:
        relative = _posix_relative(snapshot.root, directory)
        if relative == ".":
            continue
        directory_ids[relative] = facts.add_node(
            "directory",
            PurePosixPath(relative).name,
            relative,
            relative,
            identity=relative,
        )
    for relative, identifier in sorted(directory_ids.items()):
        if relative == ".":
            continue
        parent = PurePosixPath(relative).parent.as_posix()
        facts.add_edge(directory_ids.get(parent, repository_id), identifier, "contains")
    for context in snapshot.contexts:
        parent = PurePosixPath(context.relative_path).parent.as_posix()
        facts.add_edge(
            directory_ids.get(parent, repository_id),
            context.file_id,
            "contains",
        )


def _partition_paths(paths: list[Path], count: int) -> list[list[Path]]:
    if not paths:
        return []
    count = max(1, min(count, len(paths)))
    if count == 1:
        return [paths]

    weighted = [(path, max(1, path.stat().st_size)) for path in paths]
    total = sum(weight for _, weight in weighted)
    target = max(1, (total + count - 1) // count)
    partitions: list[list[Path]] = []
    current: list[Path] = []
    current_weight = 0
    for index, (path, weight) in enumerate(weighted):
        remaining_files = len(weighted) - index
        remaining_partitions = count - len(partitions)
        if (
            current
            and len(partitions) < count - 1
            and current_weight + weight > target
            and remaining_files >= remaining_partitions
        ):
            partitions.append(current)
            current = []
            current_weight = 0
        current.append(path)
        current_weight += weight
    if current:
        partitions.append(current)
    return partitions


def _reduce_fragments(
    fragments: list[ExtractionFragment],
    fan_in: int,
) -> ExtractionFragment:
    if not fragments:
        return ExtractionFragment(0, [], Facts(repository=""))
    fan_in = max(2, fan_in)
    current = fragments
    while len(current) > 1:
        next_level: list[ExtractionFragment] = []
        for start in range(0, len(current), fan_in):
            group = current[start : start + fan_in]
            merged = group[0]
            for fragment in group[1:]:
                _merge_fragment(merged, fragment)
            next_level.append(merged)
        current = next_level
    return current[0]


def _merge_fragment(
    destination: ExtractionFragment,
    source: ExtractionFragment,
) -> None:
    destination.contexts.extend(source.contexts)
    _merge_facts(destination.facts, source.facts)


def _merge_facts(destination: Facts, source: Facts) -> None:
    for name in (
        "nodes",
        "edges",
        "unresolved",
        "modules",
        "symbols",
        "symbol_kinds",
        "node_qnames",
        "functions",
        "classes",
        "lambda_ids",
        "module_bindings",
        "scope_bindings",
        "scope_parents",
    ):
        getattr(destination, name).update(getattr(source, name))
    for name in (
        "imports",
        "inheritances",
        "calls",
        "local_assignments",
        "loop_bindings",
    ):
        getattr(destination, name).extend(getattr(source, name))
    destination.dataflow_edges.update(source.dataflow_edges)
