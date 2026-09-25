mod annotation;
mod callbacks;
mod dispatch;
mod expression;
mod scope;

use std::collections::{BTreeMap, BTreeSet};

use rustpython_parser::ast;

use super::super::facts::Facts;
use super::super::model::{CallInfo, ImportInfo, LocalAssignmentInfo, LoopBindingInfo};
use super::super::source::dotted;
use super::bindings::BindingResolver;
use super::shapes::TypeShape;

pub fn resolve_calls(facts: &mut Facts) {
    let calls = facts.calls.clone();
    let resolutions = {
        let mut resolver = Resolver::new(facts);
        calls
            .iter()
            .map(|call| resolver.resolve_call(call))
            .collect::<Vec<_>>()
    };

    for (call, (targets, reason)) in calls.into_iter().zip(resolutions) {
        if targets.len() == 1 {
            let target = targets.first().expect("single call target");
            facts.add_edge(&call.owner_id, target, "calls", call.span.clone(), None);
        } else if !targets.is_empty() {
            let attributes = Some(serde_json::json!({"candidate_count": targets.len()}));
            for target in targets {
                facts.add_edge(
                    &call.owner_id,
                    &target,
                    "possible-calls",
                    call.span.clone(),
                    attributes.clone(),
                );
            }
        } else {
            facts.add_unresolved(
                &call.owner_id,
                "calls",
                &call.expression,
                &reason,
                call.span.clone(),
                dotted(&call.callee),
            );
        }
    }
}

#[derive(Default)]
struct Indexes {
    assignments: BTreeMap<(String, String), Vec<LocalAssignmentInfo>>,
    loops: BTreeMap<(String, String), Vec<LoopBindingInfo>>,
    imports: BTreeMap<(String, String), Vec<ImportInfo>>,
    field_assignments: BTreeMap<(String, String), Vec<LocalAssignmentInfo>>,
    direct_class_fields: BTreeMap<(String, String), Vec<LocalAssignmentInfo>>,
}

impl Indexes {
    fn build(facts: &Facts) -> Self {
        let mut value = Self::default();
        for assignment in &facts.local_assignments {
            value
                .assignments
                .entry((assignment.scope_id.clone(), assignment.name.clone()))
                .or_default()
                .push(assignment.clone());
            if let Some(class) = &assignment.class_qname {
                if assignment.direct_class_field {
                    value
                        .direct_class_fields
                        .entry((class.clone(), assignment.name.clone()))
                        .or_default()
                        .push(assignment.clone());
                }
                if let Some((owner, field)) = assignment.name.split_once('.')
                    && matches!(owner, "self" | "cls")
                {
                    value
                        .field_assignments
                        .entry((class.clone(), field.to_owned()))
                        .or_default()
                        .push(assignment.clone());
                }
            }
        }
        for values in value.assignments.values_mut() {
            values.sort_by_key(|item| item.start);
        }
        for binding in &facts.loop_bindings {
            value
                .loops
                .entry((binding.scope_id.clone(), binding.name.clone()))
                .or_default()
                .push(binding.clone());
        }
        for values in value.loops.values_mut() {
            values.sort_by_key(|item| item.start);
        }
        for info in &facts.imports {
            if let Some(binding) = &info.binding {
                value
                    .imports
                    .entry((info.owner_id.clone(), binding.clone()))
                    .or_default()
                    .push(info.clone());
            }
        }
        value
    }
}

struct Resolver<'a> {
    facts: &'a Facts,
    bindings: BindingResolver,
    indexes: Indexes,
    return_cache: BTreeMap<String, TypeShape>,
    return_active: BTreeSet<String>,
    parameter_cache: BTreeMap<(String, String), TypeShape>,
    parameter_active: BTreeSet<(String, String)>,
    field_cache: BTreeMap<(String, String), TypeShape>,
    base_cache: BTreeMap<String, Vec<String>>,
    decorator_argument_shapes: BTreeMap<(String, String), TypeShape>,
    effective_targets: BTreeMap<String, BTreeSet<String>>,
    direct_callers: BTreeMap<String, Vec<CallInfo>>,
}

impl<'a> Resolver<'a> {
    fn new(facts: &'a Facts) -> Self {
        let mut value = Self {
            facts,
            bindings: BindingResolver::new(facts),
            indexes: Indexes::build(facts),
            return_cache: BTreeMap::new(),
            return_active: BTreeSet::new(),
            parameter_cache: BTreeMap::new(),
            parameter_active: BTreeSet::new(),
            field_cache: BTreeMap::new(),
            base_cache: BTreeMap::new(),
            decorator_argument_shapes: BTreeMap::new(),
            effective_targets: BTreeMap::new(),
            direct_callers: BTreeMap::new(),
        };
        value.index_decorators();
        value.return_cache.clear();
        value.parameter_cache.clear();
        value.direct_callers = value.index_direct_callers();
        value
    }

    fn resolve_call(&mut self, call: &CallInfo) -> (BTreeSet<String>, String) {
        let reference = dotted(&call.callee);
        if matches!(
            reference.as_deref(),
            Some("importlib.import_module" | "import_module" | "__import__")
        ) {
            return (BTreeSet::new(), "dynamic-target".into());
        }
        if let ast::Expr::Call(callee) = &call.callee
            && dotted(&callee.func).as_deref() == Some("getattr")
            && !matches!(
                callee.args.get(1),
                Some(ast::Expr::Constant(value)) if matches!(value.value, ast::Constant::Str(_))
            )
        {
            return (BTreeSet::new(), "dynamic-target".into());
        }
        self.callable_targets(
            &call.callee,
            &call.module_name,
            call.class_qname.as_deref(),
            Some(&call.scope_id),
            u32::from(call.expression_node.range.start()),
            &mut BTreeSet::new(),
        )
    }

    fn kind(&self, id: &str) -> Option<&str> {
        self.facts.nodes.get(id).map(|node| node.kind.as_str())
    }

    fn effective_target_ids(&self, id: &str) -> BTreeSet<String> {
        self.effective_targets
            .get(id)
            .cloned()
            .unwrap_or_else(|| BTreeSet::from([id.to_owned()]))
    }

    fn shape_for_reference(&self, id: Option<String>) -> TypeShape {
        let Some(id) = id else {
            return TypeShape::default();
        };
        match self.kind(&id) {
            Some("type" | "interface" | "trait") => {
                TypeShape::direct(id.clone()).merge(TypeShape::callable(id))
            }
            Some("function" | "method") => TypeShape {
                callables: self.effective_target_ids(&id),
                ..TypeShape::default()
            },
            _ => TypeShape::default(),
        }
    }

    fn reference(
        &mut self,
        module: &str,
        class_qname: Option<&str>,
        reference: Option<&str>,
        scope: Option<&str>,
    ) -> (Option<String>, String) {
        self.bindings
            .resolve_reference(self.facts, module, class_qname, reference, scope)
    }
}
