use std::collections::BTreeSet;

use super::super::bindings::resolve_relative_module;
use super::super::shapes::TypeShape;
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn local_shape(
        &mut self,
        name: &str,
        module: &str,
        class_qname: Option<&str>,
        scope: Option<&str>,
        before: u32,
        seen: &mut BTreeSet<(String, String)>,
    ) -> TypeShape {
        let Some(scope) = scope else {
            return TypeShape::default();
        };
        let marker = (scope.to_owned(), name.to_owned());
        if !seen.insert(marker.clone()) {
            return TypeShape::default();
        }

        let mut shape = TypeShape::default();
        let mut evidence = false;

        if let Some(info) = self.facts.functions.get(scope).cloned()
            && let Some((_, annotation)) = info
                .parameters
                .iter()
                .find(|(parameter, _)| parameter == name)
        {
            evidence = true;
            shape = self
                .annotation_shape(annotation.as_ref(), module, class_qname, Some(scope))
                .merge(self.parameter_flow_shape(scope, name, seen));
        }

        let assignments = self
            .indexes
            .assignments
            .get(&(scope.to_owned(), name.to_owned()))
            .cloned()
            .unwrap_or_default();
        for index in assignments {
            let assignment = self.facts.local_assignments[index].clone();
            if assignment.end > before {
                continue;
            }
            evidence = true;
            let mut candidate = self.annotation_shape(
                assignment.annotation.as_ref(),
                &assignment.module_name,
                assignment.class_qname.as_deref(),
                Some(&assignment.scope_id),
            );
            if let Some(value) = assignment.value.as_ref() {
                candidate = candidate.merge(self.expression_shape(
                    value,
                    &assignment.module_name,
                    assignment.class_qname.as_deref(),
                    Some(&assignment.scope_id),
                    assignment.start,
                    seen,
                ));
            }
            shape = if assignment.branch_dependent {
                shape.merge(candidate)
            } else {
                candidate
            };
        }

        let loops = self
            .indexes
            .loops
            .get(&(scope.to_owned(), name.to_owned()))
            .cloned()
            .unwrap_or_default();
        for index in loops {
            let binding = self.facts.loop_bindings[index].clone();
            if binding.start > before {
                continue;
            }
            evidence = true;
            let iterable = self.expression_shape(
                &binding.iterable,
                &binding.module_name,
                binding.class_qname.as_deref(),
                Some(&binding.scope_id),
                binding.start,
                seen,
            );
            let candidate = if binding.element_index == Some(1)
                && matches!(
                    &binding.iterable,
                    rustpython_parser::ast::Expr::Call(call)
                        if matches!(
                            call.func.as_ref(),
                            rustpython_parser::ast::Expr::Attribute(attribute)
                                if attribute.attr.as_str() == "items"
                        )
                ) {
                iterable.element_shape()
            } else if binding.element_index.is_some() {
                TypeShape::default()
            } else {
                let element = iterable.element_shape();
                if element == TypeShape::default() && !iterable.runtime_reasons.is_empty() {
                    TypeShape {
                        runtime_reasons: iterable.runtime_reasons,
                        ..TypeShape::default()
                    }
                } else {
                    element
                }
            };
            shape = if binding.branch_dependent {
                shape.merge(candidate)
            } else {
                candidate
            };
        }

        if !evidence
            && let Some(parent) = self.enclosing_value_scope(scope, module)
            && parent != scope
        {
            let parent_class = self
                .facts
                .functions
                .get(&parent)
                .and_then(|info| info.class_qname.clone());
            shape = self.local_shape(
                name,
                module,
                parent_class.as_deref(),
                Some(&parent),
                before,
                seen,
            );
        }

        seen.remove(&marker);
        shape
    }

    pub(super) fn imported_value_shape(
        &mut self,
        name: &str,
        module: &str,
        scope: Option<&str>,
        seen: &mut BTreeSet<(String, String)>,
    ) -> TypeShape {
        let module_scope = self.facts.modules.get(module).cloned();
        let owners = [scope.map(str::to_owned), module_scope]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut shape = TypeShape::default();

        for owner in owners {
            let infos = self
                .indexes
                .imports
                .get(&(owner, name.to_owned()))
                .cloned()
                .unwrap_or_default();
            for index in infos {
                let info = self.facts.imports[index].clone();
                let Some(target_name) = info.target_name.as_deref() else {
                    continue;
                };
                let requested = resolve_relative_module(&info);
                let target_module =
                    self.bindings
                        .resolve_module_name(self.facts, &requested, &info.module_name);
                let Some(target_module) = target_module else {
                    continue;
                };
                let Some(target_scope) = self.facts.modules.get(&target_module).cloned() else {
                    continue;
                };
                let marker = (
                    "imported-value".to_owned(),
                    format!("{target_module}:{target_name}"),
                );
                if !seen.insert(marker.clone()) {
                    continue;
                }
                shape = shape.merge(self.local_shape(
                    target_name,
                    &target_module,
                    None,
                    Some(&target_scope),
                    u32::MAX,
                    seen,
                ));
                seen.remove(&marker);
            }
        }
        shape
    }

    fn enclosing_value_scope(&self, scope: &str, module: &str) -> Option<String> {
        let mut parent = self.facts.scope_parents.get(scope).cloned();
        while let Some(id) = parent {
            if matches!(self.kind(&id), Some("function" | "method" | "module")) {
                return Some(id);
            }
            parent = self.facts.scope_parents.get(&id).cloned();
        }
        self.facts.modules.get(module).cloned()
    }
}
