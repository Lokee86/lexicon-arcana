use rustpython_parser::{Parse, ast};

use super::super::super::source::dotted;
use super::super::shapes::{
    TypeShape, mapping_origin, sequence_origin, union_origin, wrapper_origin,
};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn annotation_shape(
        &mut self,
        annotation: Option<&ast::Expr>,
        module: &str,
        class_qname: Option<&str>,
        scope: Option<&str>,
    ) -> TypeShape {
        let Some(annotation) = annotation else {
            return TypeShape::default();
        };

        if let ast::Expr::Constant(value) = annotation
            && let ast::Constant::Str(source) = &value.value
        {
            return ast::Expr::parse(source, "<annotation>")
                .ok()
                .map(|parsed| self.annotation_shape(Some(&parsed), module, class_qname, scope))
                .unwrap_or_default();
        }

        match annotation {
            ast::Expr::BinOp(value) if matches!(value.op, ast::Operator::BitOr) => {
                let left = self.annotation_shape(Some(&value.left), module, class_qname, scope);
                let right = self.annotation_shape(Some(&value.right), module, class_qname, scope);
                left.merge(right)
            }
            ast::Expr::Subscript(value) => {
                let origin = dotted(&value.value)
                    .and_then(|name| name.rsplit('.').next().map(str::to_owned))
                    .unwrap_or_default();
                let arguments = match value.slice.as_ref() {
                    ast::Expr::Tuple(tuple) => tuple.elts.iter().collect::<Vec<_>>(),
                    item => vec![item],
                };
                let mut shapes = Vec::with_capacity(arguments.len());
                for item in arguments {
                    shapes.push(self.annotation_shape(Some(item), module, class_qname, scope));
                }
                if sequence_origin(&origin) {
                    TypeShape::elements(shapes)
                } else if mapping_origin(&origin) {
                    TypeShape::elements(shapes.into_iter().rev().take(1))
                } else if union_origin(&origin) {
                    shapes
                        .into_iter()
                        .fold(TypeShape::default(), TypeShape::merge)
                } else if wrapper_origin(&origin) {
                    shapes.into_iter().next().unwrap_or_default()
                } else if origin == "Callable" {
                    TypeShape::dynamic_callable()
                } else {
                    self.annotation_shape(Some(&value.value), module, class_qname, scope)
                }
            }
            _ => {
                let reference = dotted(annotation);
                if reference
                    .as_deref()
                    .is_some_and(|name| name == "Any" || name.ends_with(".Any"))
                {
                    return TypeShape::runtime("dynamic-target");
                }
                let (target, reason) =
                    self.reference(module, class_qname, reference.as_deref(), scope);
                if target.is_some() {
                    self.annotation_reference_shape(target)
                } else if matches!(
                    reason.as_str(),
                    "builtin-target" | "external-target" | "dynamic-target"
                ) {
                    TypeShape::runtime(&reason)
                } else {
                    TypeShape::default()
                }
            }
        }
    }

    pub(super) fn function_return_shape(
        &mut self,
        function_id: &str,
        seen: &mut std::collections::BTreeSet<(String, String)>,
    ) -> TypeShape {
        if let Some(value) = self.return_cache.get(function_id) {
            return value.clone();
        }
        if !self.return_active.insert(function_id.to_owned()) {
            return TypeShape::default();
        }
        let Some(info) = self.facts.functions.get(function_id).cloned() else {
            self.return_active.remove(function_id);
            return TypeShape::default();
        };

        let mut shape = self.annotation_shape(
            info.return_annotation.as_ref(),
            &info.module_name,
            info.class_qname.as_deref(),
            Some(&info.node_id),
        );
        if shape == TypeShape::default() {
            for expression in &info.return_expressions {
                shape = shape.merge(self.expression_shape(
                    expression,
                    &info.module_name,
                    info.class_qname.as_deref(),
                    Some(&info.node_id),
                    super::super::super::source::offset(expression),
                    seen,
                ));
            }
        }
        self.return_active.remove(function_id);
        self.return_cache
            .insert(function_id.to_owned(), shape.clone());
        shape
    }

    pub(super) fn field_shape(
        &mut self,
        class_qname: &str,
        field: &str,
        seen: &mut std::collections::BTreeSet<(String, String)>,
    ) -> TypeShape {
        let key = (class_qname.to_owned(), field.to_owned());
        if let Some(value) = self.field_cache.get(&key) {
            return value.clone();
        }
        let marker = (class_qname.to_owned(), format!("field:{field}"));
        if !seen.insert(marker.clone()) {
            return TypeShape::default();
        }

        let mut shape = TypeShape::default();
        let direct = self
            .indexes
            .direct_class_fields
            .get(&key)
            .cloned()
            .unwrap_or_default();
        for assignment in direct {
            shape = shape.merge(self.annotation_shape(
                assignment.annotation.as_ref(),
                &assignment.module_name,
                assignment.class_qname.as_deref(),
                None,
            ));
            if let Some(value) = assignment.value.as_ref() {
                shape = shape.merge(self.expression_shape(
                    value,
                    &assignment.module_name,
                    assignment.class_qname.as_deref(),
                    Some(&assignment.scope_id),
                    assignment.start,
                    seen,
                ));
            }
        }

        let fields = self
            .indexes
            .field_assignments
            .get(&key)
            .cloned()
            .unwrap_or_default();
        for assignment in fields {
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
            shape = shape.merge(candidate);
        }

        for base in self.base_qnames(class_qname) {
            shape = shape.merge(self.field_shape(&base, field, seen));
        }

        seen.remove(&marker);
        self.field_cache.insert(key, shape.clone());
        shape
    }
}
