"""Python call-graph semantic layer."""

from __future__ import annotations

from .bindings import dotted
from .callgraph_shapes import _EMPTY, TypeShape


class DispatchFlow:
    def _method_targets(self, class_id: str, method_name: str) -> set[str]:
        class_qname = self.facts.node_qnames.get(class_id)
        if not class_qname:
            return set()
        if self._kind(class_id) in {"interface", "trait"}:
            targets: set[str] = set()
            for descendant in self._descendant_ids(class_qname):
                if self._kind(descendant) in {"interface", "trait"}:
                    continue
                targets.update(self._method_targets(descendant, method_name))
            return targets
        for candidate_qname in self._mro_qnames(class_qname):
            target = self.facts.symbols.get(f"{candidate_qname}.{method_name}")
            if target and self._kind(target) == "method":
                return {target}
        return set()

    def _super_method_targets(self, class_qname: str | None, method_name: str) -> set[str]:
        if not class_qname:
            return set()
        for candidate_qname in self._mro_qnames(class_qname)[1:]:
            target = self.facts.symbols.get(f"{candidate_qname}.{method_name}")
            if target and self._kind(target) == "method":
                return {target}
        return set()

    def _runtime_base_reasons(self, class_qname: str | None) -> frozenset[str]:
        if not class_qname:
            return frozenset()
        if class_qname in self._runtime_base_cache:
            return self._runtime_base_cache[class_qname]
        info = self.facts.classes.get(class_qname)
        if info is None:
            return frozenset()
        reasons: set[str] = set()
        for base in info.bases:
            target_id, reason = self.bindings.resolve_reference(
                info.module_name, class_qname, dotted(base), None
            )
            if target_id and self._kind(target_id) in {"type", "interface", "trait"}:
                target_qname = self.facts.node_qnames.get(target_id)
                reasons.update(self._runtime_base_reasons(target_qname))
            elif reason in {"builtin-target", "external-target", "dynamic-target"}:
                reasons.add(reason)
        result = frozenset(reasons)
        self._runtime_base_cache[class_qname] = result
        return result

    def _base_qnames(self, class_qname: str) -> tuple[str, ...]:
        if class_qname in self._base_cache:
            return self._base_cache[class_qname]
        info = self.facts.classes.get(class_qname)
        bases: list[str] = []
        if info:
            for base in info.bases:
                target_id, _ = self.bindings.resolve_reference(
                    info.module_name, class_qname, dotted(base), None
                )
                if target_id and self._kind(target_id) in {"type", "interface", "trait"}:
                    target_qname = self.facts.node_qnames.get(target_id)
                    if target_qname:
                        bases.append(target_qname)
        self._base_cache[class_qname] = tuple(bases)
        return tuple(bases)

    def _mro_qnames(self, class_qname: str) -> tuple[str, ...]:
        if class_qname in self._mro_cache:
            return self._mro_cache[class_qname]
        bases = self._base_qnames(class_qname)
        if not bases:
            result = (class_qname,)
            self._mro_cache[class_qname] = result
            return result
        sequences = [list(self._mro_qnames(base)) for base in bases]
        sequences.append(list(bases))
        merged: list[str] = []
        while any(sequences):
            sequences = [sequence for sequence in sequences if sequence]
            candidate = next(
                (
                    sequence[0]
                    for sequence in sequences
                    if all(sequence[0] not in other[1:] for other in sequences)
                ),
                None,
            )
            if candidate is None:
                candidate = sequences[0][0]
            if candidate not in merged:
                merged.append(candidate)
            for sequence in sequences:
                if sequence and sequence[0] == candidate:
                    sequence.pop(0)
        result = (class_qname, *merged)
        self._mro_cache[class_qname] = result
        return result

    def _instance_type_ids(self, class_id: str) -> frozenset[str]:
        class_qname = self.facts.node_qnames.get(class_id)
        if not class_qname:
            return frozenset({class_id})
        return frozenset({class_id, *self._descendant_ids(class_qname)})

    def _descendant_ids(self, class_qname: str) -> frozenset[str]:
        if class_qname in self._descendant_cache:
            return self._descendant_cache[class_qname]
        if self._children_by_base is None:
            children: dict[str, list[str]] = {}
            for candidate_qname in self.facts.classes:
                for base_qname in self._base_qnames(candidate_qname):
                    children.setdefault(base_qname, []).append(candidate_qname)
            self._children_by_base = {
                base_qname: tuple(candidate_qnames)
                for base_qname, candidate_qnames in children.items()
            }

        descendants: set[str] = set()
        seen: set[str] = set()
        pending = list(self._children_by_base.get(class_qname, ()))
        while pending:
            candidate_qname = pending.pop()
            if candidate_qname in seen:
                continue
            seen.add(candidate_qname)
            candidate = self.facts.classes.get(candidate_qname)
            if candidate is None:
                continue
            descendants.add(candidate.node_id)
            pending.extend(self._children_by_base.get(candidate_qname, ()))

        result = frozenset(descendants)
        self._descendant_cache[class_qname] = result
        return result

    def _annotation_shape_for_reference(self, identifier: str | None) -> TypeShape:
        if identifier is None or self._kind(identifier) not in {"type", "interface", "trait"}:
            return _EMPTY
        return TypeShape(direct=self._instance_type_ids(identifier))

    def _shape_for_reference(self, identifier: str | None) -> TypeShape:
        if identifier is None:
            return _EMPTY
        kind = self._kind(identifier)
        if kind in {"type", "interface", "trait"}:
            return TypeShape(direct=frozenset({identifier}), callables=frozenset({identifier}))
        if kind in {"function", "method"}:
            return TypeShape(callables=self._effective_target_ids(identifier))
        return _EMPTY

    def _kind(self, identifier: str) -> str | None:
        record = self.facts.nodes.get(identifier)
        return record.kind if record is not None else None
