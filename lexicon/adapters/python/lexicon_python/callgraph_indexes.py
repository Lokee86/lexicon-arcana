"""Precomputed repository-scale indexes for Python callgraph resolution."""

from __future__ import annotations

from dataclasses import dataclass

from .model import Facts, ImportInfo, LocalAssignmentInfo, LoopBindingInfo


@dataclass(frozen=True)
class CallGraphIndexes:
    assignments_by_scope_name: dict[
        tuple[str, str],
        tuple[LocalAssignmentInfo, ...],
    ]
    loop_bindings_by_scope_name: dict[
        tuple[str, str],
        tuple[LoopBindingInfo, ...],
    ]
    imports_by_owner_binding: dict[
        tuple[str, str],
        tuple[ImportInfo, ...],
    ]
    field_assignments: dict[
        tuple[str, str],
        tuple[LocalAssignmentInfo, ...],
    ]


def build_callgraph_indexes(facts: Facts) -> CallGraphIndexes:
    assignments: dict[tuple[str, str], list[LocalAssignmentInfo]] = {}
    field_assignments: dict[tuple[str, str], list[LocalAssignmentInfo]] = {}
    for assignment in facts.local_assignments:
        assignments.setdefault(
            (assignment.scope_id, assignment.name),
            [],
        ).append(assignment)
        if assignment.class_qname and "." in assignment.name:
            owner, field_name = assignment.name.split(".", 1)
            if owner in {"self", "cls"}:
                field_assignments.setdefault(
                    (assignment.class_qname, field_name),
                    [],
                ).append(assignment)

    loops: dict[tuple[str, str], list[LoopBindingInfo]] = {}
    for binding in facts.loop_bindings:
        loops.setdefault((binding.scope_id, binding.name), []).append(binding)

    imports: dict[tuple[str, str], list[ImportInfo]] = {}
    for info in facts.imports:
        if info.binding:
            imports.setdefault((info.owner_id, info.binding), []).append(info)

    return CallGraphIndexes(
        assignments_by_scope_name={
            key: tuple(items) for key, items in assignments.items()
        },
        loop_bindings_by_scope_name={
            key: tuple(items) for key, items in loops.items()
        },
        imports_by_owner_binding={
            key: tuple(items) for key, items in imports.items()
        },
        field_assignments={
            key: tuple(items) for key, items in field_assignments.items()
        },
    )
