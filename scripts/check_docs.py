#!/usr/bin/env python3
"""Validate the shared Lexicon + Arcana documentation surface and local links."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
LINK_PATTERN = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")
HTML_TARGET_PATTERN = re.compile(r"(?:src|href)=[\"']([^\"']+)[\"']", re.IGNORECASE)
EXCLUDED_PARTS = {
    ".git",
    ".worktrees",
    ".workingtrees",
    ".grimoire",
    ".grimoire-eval",
    ".go-cache",
    ".lexicon",
    ".arcana",
    ".tooling",
    ".warlock",
    ".tmp",
    "bin",
    "build",
    "build-warlock-graph",
    "dist",
    "node_modules",
    "vendor",
    "__pycache__",
    ".pytest_cache",
}
REQUIRED_DOCUMENTS = (
    "README.md",
    "docs/INDEX.md",
    "docs/architecture/analysis-stack.md",
    "docs/architecture/components.md",
    "docs/architecture/system-overview.md",
    "docs/decisions/INDEX.md",
    "docs/decisions/0006-retire-grimoire-lead-with-lexicon-arcana.md",
    "docs/development/architecture-verification.md",
    "docs/development/documentation-coverage.md",
    "docs/development/behavioral-contract-matrix.md",
    "docs/reference/installation.md",
    "docs/reference/lexicon.md",
    "docs/reference/arcana.md",
    "lexicon/docs/README.md",
    "lexicon/docs/MAINTAINER_MAP.md",
    "arcana/docs/README.md",
    "arcana/docs/APPLICATION.md",
    "arcana/docs/ARCHITECTURE.md",
    "arcana/docs/MAINTAINER_MAP.md",
    "arcana/docs/DEVELOPMENT.md",
    "arcana/docs/STATUS.md",
)
RETIRED_CENTRAL_CODEMAPS = (
    "docs/architecture/codemap.md",
    "lexicon/docs/CODEMAP.md",
    "arcana/docs/CODEMAP.md",
)
RETIRED_BASELINES = (
    "docs-standard.baseline.json",
    "docs-standard.lexicon.baseline.json",
    "docs-standard.arcana.baseline.json",
)
DOCUMENTATION_CONFIGS = (
    "docs-standard.json",
    "docs-standard.lexicon.json",
    "docs-standard.arcana.json",
)
MAINTAINER_MAPS = (
    "docs/architecture/maintainer-map.md",
    "lexicon/docs/MAINTAINER_MAP.md",
    "arcana/docs/MAINTAINER_MAP.md",
)
CANONICAL_DOC_ROOTS = (
    ROOT / "docs",
    ROOT / "lexicon" / "docs",
    ROOT / "arcana" / "docs",
)
PARENT_INDEX_EXEMPT_NAMES = {"INDEX.md", "README.md"}
PARENT_INDEX_EXEMPT_PATHS = {"docs/development/recent-changes-2026-07.md"}
RESEARCH_DOCUMENTS = (
    "docs/development/agent-benchmark-findings.md",
    "docs/development/ranking-calibration-corpus.md",
    "lexicon/docs/GO_ADAPTER_VALIDATION.md",
    "lexicon/docs/LOTUSSCRIPT_ADAPTER_VALIDATION.md",
    "lexicon/docs/SEMANTIC_CORPUS_VALIDATION.md",
)
RESEARCH_SECTIONS = (
    "Research status",
    "Question",
    "Method",
    "Corpus or inputs",
    "Results",
    "Limitations",
    "Interpretation",
    "Retained artifacts",
)
PLANNING_DOCUMENTS = ("docs/planning/roadmap.md",)
PLANNING_SECTIONS = (
    "Current status",
    "Expected ownership",
    "Planned behavior",
    "Implementation sequence",
    "Acceptance criteria",
    "Open decisions",
)
ARCHITECTURE_DOCUMENTS = (
    "docs/architecture/analysis-stack.md",
    "docs/architecture/components.md",
    "docs/architecture/system-overview.md",
    "lexicon/docs/ARCHITECTURE.md",
    "arcana/docs/ARCHITECTURE.md",
)
GUIDE_DOCUMENTS = ("docs/reference/installation.md",)
GUIDE_SECTIONS = ("Prerequisites", "Expected result", "Failure and recovery")
CODE_MAP_DOCUMENTS = (
    "docs/architecture/analysis-stack.md",
    "docs/architecture/components.md",
    "docs/architecture/system-overview.md",
    "docs/development/architecture-verification.md",
    "docs/reference/arcana.md",
    "docs/reference/lexicon.md",
    "docs/development/behavioral-contract-matrix.md",
    "docs/development/documentation-coverage.md",
    "docs/development/release-workflow.md",
    "lexicon/docs/APPLICATION.md",
    "lexicon/docs/ARCHITECTURE.md",
    "lexicon/docs/DEPENDENCY_SEMANTICS.md",
    "lexicon/docs/DEVELOPMENT.md",
    "lexicon/docs/RELEASE_PACKAGING.md",
    "lexicon/docs/SEMANTIC_ACCEPTANCE.md",
    "lexicon/adapters/README.md",
    "lexicon/adapters/c-family/README.md",
    "lexicon/adapters/csharp/README.md",
    "lexicon/adapters/gdscript/README.md",
    "lexicon/adapters/generic/README.md",
    "lexicon/adapters/go/README.md",
    "lexicon/adapters/java/README.md",
    "lexicon/adapters/kotlin/README.md",
    "lexicon/adapters/lotusscript/README.md",
    "lexicon/adapters/python/README.md",
    "lexicon/adapters/ruby/README.md",
    "lexicon/adapters/rust/README.md",
    "lexicon/adapters/typescript/README.md",
    "arcana/docs/APPLICATION.md",
    "arcana/docs/ARCHITECTURE.md",
    "arcana/docs/DEVELOPMENT.md",
    "arcana/docs/LEXICON_CONTRACT.md",
    "arcana/docs/repository-snapshots.md",
    "arcana/docs/vector-index.md",
)


def excluded(path: Path) -> bool:
    for part in path.relative_to(ROOT).parts:
        if part in EXCLUDED_PARTS or part.startswith("target-v"):
            return True
    return False


def documentation_files() -> list[Path]:
    return sorted(path for path in ROOT.rglob("*.md") if path.is_file() and not excluded(path))


def local_link_target(raw: str) -> str | None:
    value = raw.strip()
    if value.startswith("<") and ">" in value:
        value = value[1 : value.index(">")]
    else:
        value = value.split(maxsplit=1)[0]
    if not value or value.startswith("#"):
        return None
    parsed = urlsplit(value)
    if parsed.scheme or parsed.netloc:
        return None
    return unquote(parsed.path) or None


def validate_required_documents() -> list[str]:
    failures = [
        f"missing required documentation: {relative}"
        for relative in REQUIRED_DOCUMENTS
        if not (ROOT / relative).is_file()
    ]
    failures.extend(
        f"retired centralized codemap still exists: {relative}"
        for relative in RETIRED_CENTRAL_CODEMAPS
        if (ROOT / relative).exists()
    )
    failures.extend(
        f"documentation baseline is not permitted: {relative}"
        for relative in RETIRED_BASELINES
        if (ROOT / relative).exists()
    )
    return failures


def validate_zero_baseline_configuration() -> list[str]:
    failures: list[str] = []
    for relative in DOCUMENTATION_CONFIGS:
        path = ROOT / relative
        if not path.is_file():
            continue
        try:
            config = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as error:
            failures.append(f"{relative}: invalid JSON: {error}")
            continue
        if config.get("baseline"):
            failures.append(f"{relative}: documentation baselines are not permitted")
    return failures


def has_heading(text: str, heading: str) -> bool:
    return re.search(rf"(?m)^## {re.escape(heading)}\s*$", text) is not None


def validate_parent_indexes() -> list[str]:
    failures: list[str] = []
    for root in CANONICAL_DOC_ROOTS:
        for document in sorted(root.rglob("*.md")):
            relative = document.relative_to(ROOT).as_posix()
            if document.name in PARENT_INDEX_EXEMPT_NAMES or relative in PARENT_INDEX_EXEMPT_PATHS:
                continue
            text = document.read_text(encoding="utf-8")
            if re.search(r"(?m)^Parent index:\s+\S", text) is None:
                failures.append(f"{relative}: missing Parent index")
    return failures


def validate_type_specific_sections() -> list[str]:
    failures: list[str] = []
    requirements = (
        (RESEARCH_DOCUMENTS, RESEARCH_SECTIONS),
        (PLANNING_DOCUMENTS, PLANNING_SECTIONS),
        (ARCHITECTURE_DOCUMENTS, ("Code map", "Tests")),
        (GUIDE_DOCUMENTS, GUIDE_SECTIONS),
    )
    for documents, sections in requirements:
        for relative in documents:
            path = ROOT / relative
            if not path.is_file():
                failures.append(f"missing typed documentation: {relative}")
                continue
            text = path.read_text(encoding="utf-8")
            for section in sections:
                if not has_heading(text, section):
                    failures.append(f"{relative}: missing type-specific section '## {section}'")
    return failures


def validate_code_maps() -> list[str]:
    failures: list[str] = []
    for relative in CODE_MAP_DOCUMENTS:
        document = ROOT / relative
        if not document.is_file():
            failures.append(f"missing code-map document: {relative}")
            continue
        text = document.read_text(encoding="utf-8")
        if re.search(r"(?m)^## Code map\s*$", text) is None:
            failures.append(f"{relative}: missing focused '## Code map' section")
    for relative in MAINTAINER_MAPS:
        document = ROOT / relative
        if not document.is_file():
            continue
        text = document.read_text(encoding="utf-8")
        if re.search(r"(?m)^## Code map\s*$", text) is not None:
            failures.append(
                f"{relative}: maintainer map must route to focused code maps, not contain one"
            )
        line_count = len(text.splitlines())
        if line_count > 120:
            failures.append(
                f"{relative}: maintainer map is too large ({line_count} lines; maximum 120)"
            )
    return failures


def validate_local_links() -> list[str]:
    failures: list[str] = []
    for document in documentation_files():
        text = document.read_text(encoding="utf-8")
        raw_targets = [match.group(1) for match in LINK_PATTERN.finditer(text)]
        raw_targets.extend(match.group(1) for match in HTML_TARGET_PATTERN.finditer(text))
        for raw_target in raw_targets:
            target = local_link_target(raw_target)
            if target is None:
                continue
            resolved = (document.parent / target).resolve()
            if not resolved.exists():
                failures.append(
                    f"{document.relative_to(ROOT)}: missing local link target {target!r}"
                )
    return failures


def validate_index_visibility() -> list[str]:
    failures: list[str] = []
    required_links = {
        ROOT / "README.md": (
            "docs/INDEX.md",
            "docs/reference/lexicon.md",
            "docs/reference/arcana.md",
        ),
        ROOT / "lexicon" / "docs" / "README.md": ("MAINTAINER_MAP.md",),
        ROOT / "arcana" / "docs" / "README.md": (
            "APPLICATION.md",
            "ARCHITECTURE.md",
            "MAINTAINER_MAP.md",
            "DEVELOPMENT.md",
            "STATUS.md",
        ),
    }
    for document, targets in required_links.items():
        if not document.is_file():
            continue
        text = document.read_text(encoding="utf-8")
        for target in targets:
            if f"]({target})" not in text:
                failures.append(
                    f"{document.relative_to(ROOT)}: required documentation link is not indexed: {target}"
                )
    return failures


def validate_repository() -> list[str]:
    return (
        validate_required_documents()
        + validate_zero_baseline_configuration()
        + validate_parent_indexes()
        + validate_type_specific_sections()
        + validate_code_maps()
        + validate_local_links()
        + validate_index_visibility()
    )


def main() -> int:
    failures = validate_repository()
    if failures:
        print("documentation validation failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print(f"Documentation validation passed for {len(documentation_files())} Markdown files.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
