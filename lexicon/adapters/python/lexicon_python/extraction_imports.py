"""Python extraction visitor layer."""

from __future__ import annotations

import ast

from .contract import expression_text, span
from .model import ImportInfo


class ImportFlow:
    def visit_Import(self, node: ast.Import) -> None:
        for alias in node.names:
            self._add_import(node, alias.name, alias.asname or alias.name.split(".")[0], 0, None, False)

    def visit_ImportFrom(self, node: ast.ImportFrom) -> None:
        module = node.module or ""
        for alias in node.names:
            binding = None if alias.name == "*" else (alias.asname or alias.name)
            self._add_import(node, module, binding, node.level, alias.name, alias.name == "*")

    def _add_import(
        self,
        statement: ast.AST,
        target_module: str,
        binding: str | None,
        relative_level: int,
        target_name: str | None,
        star: bool,
    ) -> None:
        self.import_index += 1
        statement_span = span(statement, self.context.relative_path, self.context.source)
        expression = expression_text(statement, self.context.source)
        if target_name and target_name != "*":
            expression = f"{expression} [{target_name}]"
        qualified_name = (
            f"{self.context.module_name}::import:{statement_span.get('start_line', 0) if statement_span else 0}:"
            f"{self.import_index}:{binding or target_module}"
        )
        identifier = self.facts.add_node(
            "import",
            binding or target_name or target_module,
            self.context.relative_path,
            qualified_name,
            identity=qualified_name,
            record_span=statement_span,
            attributes={"expression": expression},
        )
        self.facts.add_edge(self.owner_id, identifier, "defines", record_span=statement_span)
        self.facts.imports.append(
            ImportInfo(
                module_name=self.context.module_name,
                owner_id=self.owner_id,
                node_id=identifier,
                expression=expression,
                binding=binding,
                target_module=target_module,
                target_name=target_name,
                relative_level=relative_level,
                star=star,
                is_package=self.context.relative_path.endswith("/__init__.py")
                or self.context.relative_path == "__init__.py",
            )
        )
        if binding:
            self.facts.scope_bindings[(self.owner_id, binding)] = (None, "unresolved")
            if self.owner_id == self.context.module_id:
                self.facts.module_bindings[(self.context.module_name, binding)] = (None, "unresolved")
