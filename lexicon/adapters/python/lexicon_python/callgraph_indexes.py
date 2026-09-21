"""Precomputed repository-scale indexes for Python callgraph resolution."""

from __future__ import annotations

from dataclasses import dataclass

from .model import Facts, ImportInfo, LocalAssignmentInfo, LoopBindingInfo


@dataclass(frozen=True, slots=True)
class CallGraphIndexes:
    assignments_by_scope_name: dict[
        tuple[str, str],
        list[LocalAssignmentInfo],
    ]
    loop_bindings_by_scope_name: dict[
        tuple[str, str],
        list[LoopBindingInfo],
    ]
    imports_by_owner_binding: dict[
        tuple[str, str],
        list[ImportInfo],
    ]
    field_assignments: dict[
        tuple[str, str],
        list[LocalAssignmentInfo],
    ]
    direct_class_fields: dict[
        tuple[str, str],
        list[LocalAssignmentInfo],
    ]


def build_callgraph_indexes(facts: Facts) -> CallGraphIndexes:
    assignments: dict[tuple[str, str], list[LocalAssignmentInfo]] = {}
    field_assignments: dict[tuple[str, str], list[LocalAssignmentInfo]] = {}
    direct_class_fields: dict[tuple[str, str], list[LocalAssignmentInfo]] = {}
    for assignment in facts.local_assignments:
        assignments.setdefault(
            (assignment.scope_id, assignment.name),
            [],
        ).append(assignment)
        if assignment.class_qname and assignment.direct_class_field:
            direct_class_fields.setdefault(
                (assignment.class_qname, assignment.name),
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
        assignments_by_scope_name=assignments,
        loop_bindings_by_scope_name=loops,
        imports_by_owner_binding=imports,
        field_assignments=field_assignments,
        direct_class_fields=direct_class_fields,
    )
