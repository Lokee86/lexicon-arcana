"""Repository-local import and name binding resolution."""

from __future__ import annotations

import ast

from .contract import BUILTINS
from .model import Facts, ImportInfo


def dotted(node: ast.AST | None) -> str | None:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        parent = dotted(node.value)
        return f"{parent}.{node.attr}" if parent else None
    return None


def resolve_relative_module(info: ImportInfo) -> str:
    if info.relative_level == 0:
        return info.target_module or ""
    current_parts = info.module_name.split(".") if info.module_name else []
    package_parts = current_parts if info.is_package else current_parts[:-1]
    remove = max(info.relative_level - 1, 0)
    if remove:
        package_parts = package_parts[:-remove] if remove <= len(package_parts) else []
    suffix = (info.target_module or "").split(".") if info.target_module else []
    return ".".join(package_parts + suffix)


def _common_prefix_length(left: list[str], right: list[str]) -> int:
    count = 0
    for left_part, right_part in zip(left, right):
        if left_part != right_part:
            break
        count += 1
    return count


class BindingResolver:
    def __init__(self, facts: Facts) -> None:
        self.facts = facts
        self._modules_by_suffix = self._index_module_suffixes()
        self._module_resolution_cache: dict[tuple[str, str], str | None] = {}
        self._reference_cache: dict[
            tuple[str, str | None, str | None, str | None],
            tuple[str | None, str],
        ] = {}

    def _index_module_suffixes(self) -> dict[str, tuple[str, ...]]:
        matches: dict[str, list[str]] = {}
        for name in self.facts.modules:
            parts = name.split(".")
            for index in range(len(parts)):
                suffix = ".".join(parts[index:])
                matches.setdefault(suffix, []).append(name)
        return {suffix: tuple(names) for suffix, names in matches.items()}

    def module_matches(self, requested: str) -> list[str]:
        return list(self._modules_by_suffix.get(requested, ()))

    def nearest_module(self, matches: list[str], source_module: str) -> str | None:
        if not matches:
            return None
        source_package = source_module.split(".")[:-1]
        scores = {
            match: _common_prefix_length(source_package, match.split(".")[:-1])
            for match in matches
        }
        best_score = max(scores.values())
        best = [match for match, score in scores.items() if score == best_score]
        return best[0] if len(best) == 1 else None

    def resolve_module_name(self, requested: str, source_module: str) -> str | None:
        key = (requested, source_module)
        if key in self._module_resolution_cache:
            return self._module_resolution_cache[key]
        matches = self.module_matches(requested)
        result = (
            requested
            if requested in matches
            else self.nearest_module(matches, source_module)
        )
        self._module_resolution_cache[key] = result
        return result

    def resolve_import(self, info: ImportInfo) -> tuple[str | None, str]:
        if info.star:
            return None, "unsupported-form"
        requested_module = resolve_relative_module(info)
        module_name = self.resolve_module_name(requested_module, info.module_name)
        if info.target_name is None:
            target = self.facts.modules.get(module_name) if module_name else None
            return (target, "") if target else (None, "external-target")
        matching_targets: dict[str, str] = {}
        for candidate_module in self.module_matches(requested_module):
            symbol = f"{candidate_module}.{info.target_name}"
            if symbol in self.facts.symbols:
                matching_targets[candidate_module] = self.facts.symbols[symbol]
                continue
            if symbol in self.facts.modules:
                matching_targets[candidate_module] = self.facts.modules[symbol]
                continue
            reexport = self.facts.module_bindings.get((candidate_module, info.target_name))
            if reexport and reexport[0]:
                matching_targets[candidate_module] = reexport[0]
        selected_module = self.nearest_module(list(matching_targets), info.module_name)
        if selected_module:
            return matching_targets[selected_module], ""
        if module_name:
            return None, "missing-target"
        raw_symbol = f"{requested_module}.{info.target_name}" if requested_module else info.target_name
        if raw_symbol in self.facts.symbols:
            return self.facts.symbols[raw_symbol], ""
        if raw_symbol in self.facts.modules:
            return self.facts.modules[raw_symbol], ""
        return None, "external-target"

    def resolve_imports(self) -> list[tuple[str | None, str]]:
        imports = self.facts.imports
        results: list[tuple[str | None, str]] = [
            (None, "unresolved")
        ] * len(imports)
        if not imports:
            return results

        # Re-export resolution is a fixed-point problem, but only module-level
        # bindings can affect another import. Build the reverse dependency
        # graph once and revisit only imports whose candidate binding changed.
        dependents: dict[tuple[str, str], list[int]] = {}
        for index, info in enumerate(imports):
            if not info.target_name:
                continue
            requested_module = resolve_relative_module(info)
            for candidate_module in self.module_matches(requested_module):
                dependents.setdefault(
                    (candidate_module, info.target_name),
                    [],
                ).append(index)

        queue = list(range(len(imports)))
        queued = [True] * len(imports)
        cursor = 0
        while cursor < len(queue):
            index = queue[cursor]
            cursor += 1
            queued[index] = False
            info = imports[index]
            result = self.resolve_import(info)
            results[index] = result
            if not info.binding:
                continue

            scope_key = (info.owner_id, info.binding)
            if self.facts.scope_bindings.get(scope_key) != result:
                self.facts.scope_bindings[scope_key] = result

            module_id = self.facts.modules.get(info.module_name)
            if info.owner_id != module_id:
                continue

            module_key = (info.module_name, info.binding)
            if self.facts.module_bindings.get(module_key) == result:
                continue
            self.facts.module_bindings[module_key] = result

            for dependent in dependents.get(module_key, ()):
                if not queued[dependent]:
                    queue.append(dependent)
                    queued[dependent] = True

        # Import resolution is the only phase that mutates the binding maps.
        # Drop any speculative reference results if this resolver was queried
        # before those maps reached their fixed point.
        self._reference_cache.clear()
        return results

    def resolve_reference(
        self,
        module_name: str,
        class_qname: str | None,
        reference: str | None,
        scope_id: str | None = None,
    ) -> tuple[str | None, str]:
        key = (module_name, class_qname, reference, scope_id)
        cached = self._reference_cache.get(key)
        if cached is not None:
            return cached
        result = self._resolve_reference_uncached(
            module_name,
            class_qname,
            reference,
            scope_id,
        )
        self._reference_cache[key] = result
        return result

    def _resolve_reference_uncached(
        self,
        module_name: str,
        class_qname: str | None,
        reference: str | None,
        scope_id: str | None = None,
    ) -> tuple[str | None, str]:
        if not reference:
            return None, "unsupported-form"
        if reference in BUILTINS:
            return None, "builtin-target"
        parts = reference.split(".")
        if class_qname and parts[0] in {"self", "cls"} and len(parts) == 2:
            candidate = f"{class_qname}.{parts[1]}"
            if candidate in self.facts.symbols:
                return self.facts.symbols[candidate], ""
        binding = None
        if scope_id:
            binding = self.facts.scope_bindings.get((scope_id, parts[0]))
        if binding is None:
            binding = self.facts.module_bindings.get((module_name, parts[0]))
        if binding and binding[0]:
            base_qname = self.facts.node_qnames.get(binding[0])
            if base_qname:
                candidate = ".".join([base_qname, *parts[1:]])
                if candidate in self.facts.symbols:
                    return self.facts.symbols[candidate], ""
                if candidate in self.facts.modules:
                    return self.facts.modules[candidate], ""
            if len(parts) == 1:
                return binding[0], ""
        candidates = [".".join([module_name, *parts]), reference]
        if scope_id:
            scope_qname = self.facts.node_qnames.get(scope_id)
            while scope_qname and scope_qname.startswith(module_name):
                candidates.insert(0, f"{scope_qname}.{reference}")
                if scope_qname == module_name or "." not in scope_qname:
                    break
                scope_qname = scope_qname.rsplit(".", 1)[0]
        if class_qname and len(parts) == 1:
            candidates.insert(0, f"{class_qname}.{reference}")
        for candidate in candidates:
            if candidate in self.facts.symbols:
                return self.facts.symbols[candidate], ""
            if candidate in self.facts.modules:
                return self.facts.modules[candidate], ""
        if binding and binding[1] == "external-target":
            return None, "external-target"
        return None, "missing-target"
