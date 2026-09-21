"""Repository walking, Python source loading, and module discovery."""

from __future__ import annotations

import ast
import io
import os
import tokenize
from pathlib import Path, PurePosixPath

from .contract import EXCLUDED_DIRECTORIES
from .model import FileContext, RepositorySnapshot


def _posix_relative(root: Path, path: Path) -> str:
    relative = path.relative_to(root).as_posix()
    return relative or "."


def _module_name(root_name: str, relative_path: str) -> str:
    parts = list(PurePosixPath(relative_path).parts)
    leaf = parts.pop()
    stem = leaf[:-3] if leaf.endswith(".py") else leaf
    if stem == "__init__":
        if parts:
            return ".".join(parts)
        return root_name
    parts.append(stem)
    return ".".join(parts)


def _name_from_dotted(value: str) -> str:
    return value.rsplit(".", 1)[-1]


def _scan_paths(root: Path) -> tuple[list[Path], list[Path]]:
    directories: list[Path] = [root]
    python_files: list[Path] = []
    for current, dirnames, filenames in os.walk(root, topdown=True, followlinks=False):
        dirnames[:] = sorted(name for name in dirnames if name not in EXCLUDED_DIRECTORIES)
        current_path = Path(current)
        directories.extend(current_path / name for name in dirnames)
        python_files.extend(
            current_path / name for name in sorted(filenames) if name.lower().endswith(".py")
        )
    return sorted(set(directories)), sorted(set(python_files))


def _read_source(path: Path) -> tuple[str, bytes]:
    data = path.read_bytes()
    encoding, _ = tokenize.detect_encoding(io.BytesIO(data).readline)
    with io.TextIOWrapper(io.BytesIO(data), encoding=encoding, newline=None) as handle:
        source = handle.read()
    return source, data


def inventory(repo: Path) -> tuple[Path, str, list[Path], list[Path]]:
    root = repo.expanduser().resolve()
    if not root.is_dir():
        raise NotADirectoryError(f"repository is not a directory: {repo}")
    directories, python_files = _scan_paths(root)
    return root, root.name, directories, python_files


def load_context(root: Path, repository: str, path: Path) -> FileContext:
    relative = _posix_relative(root, path)
    module_name = _module_name(repository, relative)
    data = path.read_bytes()
    try:
        encoding, _ = tokenize.detect_encoding(io.BytesIO(data).readline)
        with io.TextIOWrapper(io.BytesIO(data), encoding=encoding, newline=None) as handle:
            source = handle.read()
        tree: ast.AST | None = ast.parse(source, filename=relative)
        parse_error = None
    except (SyntaxError, UnicodeDecodeError, tokenize.TokenError) as error:
        source = ""
        tree = None
        parse_error = type(error).__name__
    return FileContext(
        root=root,
        path=path,
        relative_path=relative,
        module_name=module_name,
        source=source,
        lines=source.splitlines(),
        tree=tree,
        file_id="",
        module_id="",
        data=data,
        parse_error=parse_error,
    )


def discover(repo: Path) -> RepositorySnapshot:
    root, repository, directories, python_files = inventory(repo)
    contexts = [load_context(root, repository, path) for path in python_files]
    return RepositorySnapshot(root, repository, directories, contexts)


__all__ = [
    "discover",
    "inventory",
    "load_context",
    "_name_from_dotted",
    "_posix_relative",
]
