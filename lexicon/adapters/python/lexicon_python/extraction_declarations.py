"""Python extraction visitor layer."""

from __future__ import annotations

import ast

from .contract import expression_text, span
from .model import ClassInfo, FunctionInfo, InheritanceInfo


class DeclarationFlow:
    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        nested_names = [name for name, _ in self.lexical_stack]
        nested_names.append(node.name)
        qname = f"{self.context.module_name}.{'.'.join(nested_names)}"
        kind = "interface" if self._is_contract(node) else "type"
        identifier = self.facts.add_node(
            kind,
            node.name,
            self.context.relative_path,
            qname,
            record_span=span(node, self.context.relative_path, self.context.lines),
            attributes={
                **self._attributes(node),
                **({"bases": sorted(expression_text(base, self.context.source) for base in node.bases)} if node.bases else {}),
            },
        )
        self.facts.symbols[qname] = identifier
        self.facts.symbol_kinds[qname] = kind
        self.facts.scope_parents[identifier] = self.owner_id
        self.facts.classes[qname] = ClassInfo(self.context.module_name, qname, identifier, node)
        self.facts.add_edge(
            self.owner_id,
            identifier,
            "defines",
            record_span=span(node, self.context.relative_path, self.context.lines),
        )
        self.class_stack.append((qname, node.name))
        self.lexical_stack.append((node.name, "class"))
        self.owner_stack.append(identifier)
        for base in node.bases:
            self._inheritance(identifier, qname, base)
        for statement in node.body:
            self.visit(statement)
        self.owner_stack.pop()
        self.lexical_stack.pop()
        self.class_stack.pop()

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        self._visit_function(node)

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        self._visit_function(node)

    def _visit_function(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> None:
        prefix = [self.context.module_name]
        prefix.extend(name for name, _ in self.lexical_stack)
        prefix.append(node.name)
        qname = ".".join(prefix)
        kind = "method" if self.lexical_stack and self.lexical_stack[-1][1] == "class" else "function"
        identifier = self.facts.add_node(
            kind,
            node.name,
            self.context.relative_path,
            qname,
            record_span=span(node, self.context.relative_path, self.context.lines),
            attributes=self._attributes(node),
        )
        self.facts.symbols[qname] = identifier
        self.facts.symbol_kinds[qname] = kind
        self.facts.scope_parents[identifier] = self.owner_id
        parameters = {
            argument.arg: argument.annotation
            for argument in [
                *node.args.posonlyargs,
                *node.args.args,
                *node.args.kwonlyargs,
            ]
        }
        if node.args.vararg is not None:
            parameters[node.args.vararg.arg] = node.args.vararg.annotation
        if node.args.kwarg is not None:
            parameters[node.args.kwarg.arg] = node.args.kwarg.annotation
        self.facts.functions[identifier] = FunctionInfo(
            self.context.module_name,
            qname,
            identifier,
            self.class_qname,
            node,
            parameters,
            node.returns,
        )
        self.facts.add_edge(
            self.owner_id,
            identifier,
            "defines",
            record_span=span(node, self.context.relative_path, self.context.lines),
        )
        self.function_stack.append((qname, node.name))
        self.lexical_stack.append((node.name, "function"))
        self.owner_stack.append(identifier)
        previous_control_flow_depth = self.control_flow_depth
        self.control_flow_depth = 0
        for name in parameters:
            self._declare_data_symbol(name, "parameter", node)
        self._predeclare_function_locals(node)
        for statement in node.body:
            self.visit(statement)
        self.control_flow_depth = previous_control_flow_depth
        self.owner_stack.pop()
        self.lexical_stack.pop()
        self.function_stack.pop()

    def _predeclare_function_locals(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> None:
        def visit_scope(current: ast.AST) -> None:
            for child in ast.iter_child_nodes(current):
                if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef)):
                    continue
                inspect_node(child)
                visit_scope(child)

        def inspect_node(child: ast.AST) -> None:
            if isinstance(child, (ast.Assign, ast.AnnAssign, ast.AugAssign, ast.NamedExpr)):
                targets = child.targets if isinstance(child, ast.Assign) else [child.target]
                for target in targets:
                    for value in ast.walk(target):
                        if isinstance(value, ast.Name):
                            self._declare_data_symbol(value.id, node=value)
            elif isinstance(child, (ast.For, ast.AsyncFor)):
                for value in ast.walk(child.target):
                    if isinstance(value, ast.Name):
                        self._declare_data_symbol(value.id, node=value)

        visit_scope(node)

    def visit_Lambda(self, node: ast.Lambda) -> None:
        owner_qname = self.facts.node_qnames.get(self.owner_id, self.context.module_name)
        name = f"<lambda>@{node.lineno}:{node.col_offset + 1}"
        qname = f"{owner_qname}.{name}"
        identifier = self.facts.add_node(
            "function",
            name,
            self.context.relative_path,
            qname,
            record_span=span(node, self.context.relative_path, self.context.lines),
            attributes={"lambda": True},
        )
        parameters = {
            argument.arg: argument.annotation
            for argument in [
                *node.args.posonlyargs,
                *node.args.args,
                *node.args.kwonlyargs,
            ]
        }
        if node.args.vararg is not None:
            parameters[node.args.vararg.arg] = node.args.vararg.annotation
        if node.args.kwarg is not None:
            parameters[node.args.kwarg.arg] = node.args.kwarg.annotation
        self.facts.functions[identifier] = FunctionInfo(
            self.context.module_name,
            qname,
            identifier,
            self.class_qname,
            node,
            parameters,
            None,
        )
        self.facts.scope_parents[identifier] = self.owner_id
        self.facts.lambda_ids[(self.context.module_name, node.lineno, node.col_offset)] = identifier
        self.facts.add_edge(
            self.owner_id,
            identifier,
            "defines",
            record_span=span(node, self.context.relative_path, self.context.lines),
        )
        self.function_stack.append((qname, name))
        self.lexical_stack.append((name, "function"))
        self.owner_stack.append(identifier)
        for name in parameters:
            self._declare_data_symbol(name, "parameter", node)
        self.visit(node.body)
        self.owner_stack.pop()
        self.lexical_stack.pop()
        self.function_stack.pop()

    def _inheritance(self, source_id: str, class_qname: str, base: ast.expr) -> None:
        self.facts.inheritances.append(
            InheritanceInfo(
                source_id=source_id,
                module_name=self.context.module_name,
                class_qname=class_qname,
                base=base,
                source=self.context.source,
                path=self.context.relative_path,
                lines=self.context.lines,
            )
        )

    def _is_contract(self, node: ast.ClassDef) -> bool:
        bases = {expression_text(base, self.context.source) for base in node.bases}
        if any(base.split(".")[-1] in {"Protocol", "ABC", "Interface", "Trait"} for base in bases):
            return True
        return any(
            expression_text(decorator, self.context.source).split(".")[-1] in {"runtime_checkable", "abstractclass"}
            for decorator in node.decorator_list
        )
