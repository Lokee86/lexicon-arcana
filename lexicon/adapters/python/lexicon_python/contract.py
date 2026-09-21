"""Lexicon facts v1 contract primitives for the Python adapter."""

from __future__ import annotations

import ast
import builtins as python_builtins
import hashlib
from functools import lru_cache
from typing import Any

LANGUAGE = "python"
SCHEMA_VERSION = 1

EXCLUDED_DIRECTORIES = frozenset(
    {
        ".git",
        ".worktrees",
        ".workingtrees",
        ".ddocs",
        ".lexicon",
        ".arcana",
        ".grimoire",
        ".pitlord",
        ".cantrip",
        ".homunculus",
        ".incubus",
        ".ritual",
        ".warlock",
        ".bundle",
        ".eggs",
        ".mypy_cache",
        ".next",
        ".nox",
        ".pytest_cache",
        ".ruff_cache",
        ".tox",
        ".venv",
        "__pycache__",
        "build",
        "dist",
        "env",
        "node_modules",
        "site-packages",
        "target",
        "vendor",
        "venv",
    }
)

BUILTINS = frozenset(dir(python_builtins))


def digest(value: str | bytes) -> str:
    payload = value.encode("utf-8") if isinstance(value, str) else value
    return f"sha256:{hashlib.sha256(payload).hexdigest()}"


def node_id(kind: str, identity: str) -> str:
    return digest(f"lexicon:v1\0{LANGUAGE}\0{kind}\0{identity}")


def content_id(data: bytes) -> str:
    return digest(data)


@lru_cache(maxsize=None)
def _source_byte_index(source: str) -> tuple[bytes, tuple[int, ...]]:
    encoded = source.encode("utf-8")
    starts = [0]
    offset = 0
    while True:
        newline = encoded.find(b"\n", offset)
        if newline < 0:
            break
        offset = newline + 1
        starts.append(offset)
    return encoded, tuple(starts)


def expression_text(node: ast.AST, source: str) -> str:
    try:
        lineno = node.lineno
        end_lineno = node.end_lineno
        col_offset = node.col_offset
        end_col_offset = node.end_col_offset
    except AttributeError:
        lineno = end_lineno = col_offset = end_col_offset = None

    if None not in (lineno, end_lineno, col_offset, end_col_offset):
        encoded, starts = _source_byte_index(source)
        if 1 <= lineno <= len(starts) and 1 <= end_lineno <= len(starts):
            start = starts[lineno - 1] + col_offset
            end = starts[end_lineno - 1] + end_col_offset
            if 0 <= start <= end <= len(encoded):
                segment = encoded[start:end].decode("utf-8")
                if segment:
                    return segment.strip()
    try:
        return ast.unparse(node)
    except (AttributeError, ValueError):
        return type(node).__name__


def _column(line: str, offset: int) -> int:
    """Convert AST's UTF-8 byte offset to a one-based character column."""
    prefix = line.encode("utf-8")[:offset]
    return len(prefix.decode("utf-8", errors="ignore")) + 1


def span(node: ast.AST, path: str, lines: list[str]) -> dict[str, Any] | None:
    lineno = getattr(node, "lineno", None)
    col_offset = getattr(node, "col_offset", None)
    end_lineno = getattr(node, "end_lineno", None)
    end_col_offset = getattr(node, "end_col_offset", None)
    if None in (lineno, col_offset, end_lineno, end_col_offset):
        return None
    if not (1 <= lineno <= len(lines) and 1 <= end_lineno <= len(lines)):
        return None
    return {
        "end_column": _column(lines[end_lineno - 1], end_col_offset),
        "end_line": end_lineno,
        "path": path,
        "start_column": _column(lines[lineno - 1], col_offset),
        "start_line": lineno,
    }
