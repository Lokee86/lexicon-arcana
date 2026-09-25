use super::{
    model::{CallObservation, Declaration, RepositoryModel},
    pointer_aliases::is_function_pointer,
    receiver_resolution::container_key,
    resolution::DeclarationIndex,
    syntax::last_qualified_part,
};
use std::collections::BTreeMap;

pub struct IndirectCallIndex<'a> {
    by_pointer: BTreeMap<String, BTreeMap<String, &'a Declaration>>,
}

impl<'a> IndirectCallIndex<'a> {
    pub fn build(model: &'a RepositoryModel, index: &'a DeclarationIndex<'a>) -> Self {
        let mut result = Self {
            by_pointer: BTreeMap::new(),
        };

        for file in &model.files {
            for declaration in &file.declarations {
                if !is_function_pointer(declaration) {
                    continue;
                }
                let target = declaration
                    .attributes
                    .get("pointer_target")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                if target.is_empty() {
                    continue;
                }
                for callable in resolve_callable_reference(
                    index,
                    target,
                    &declaration.container_qualified,
                    &declaration.path,
                ) {
                    result.add(&declaration.id, callable);
                }
            }
        }

        for file in &model.files {
            for binding in &file.pointer_bindings {
                let observation = CallObservation {
                    source_id: binding.source_id.clone(),
                    source_scope: binding.source_scope.clone(),
                    path: binding.path.clone(),
                    expression: String::new(),
                    candidate: binding.candidate.clone(),
                    arguments: Vec::new(),
                    member: binding.member,
                    receiver: String::new(),
                    receiver_type_id: String::new(),
                    span: binding.span.clone(),
                };
                let pointers = resolve_pointer_declarations(index, &observation);
                if binding.member && pointers.len() != 1 {
                    continue;
                }
                for callable in resolve_callable_reference(
                    index,
                    &binding.target,
                    &binding.source_scope,
                    &binding.path,
                ) {
                    for pointer in &pointers {
                        result.add(&pointer.id, callable);
                    }
                }
            }

            for observation in &file.calls {
                if observation.arguments.is_empty() {
                    continue;
                }
                let callees = direct_callable_targets(index, observation);
                if callees.len() != 1 {
                    continue;
                }
                let parameters = index
                    .by_callable_parameters
                    .get(&callees[0].id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                for (argument_index, argument) in observation.arguments.iter().enumerate() {
                    if argument.is_empty() || argument_index >= parameters.len() {
                        continue;
                    }
                    let parameter = parameters[argument_index];
                    if !is_function_pointer(parameter) {
                        continue;
                    }
                    for callable in resolve_callable_reference(
                        index,
                        argument,
                        &observation.source_scope,
                        &observation.path,
                    ) {
                        result.add(&parameter.id, callable);
                    }
                }
            }
        }

        result
    }

    fn add(&mut self, pointer_id: &str, target: &'a Declaration) {
        self.by_pointer
            .entry(pointer_id.to_owned())
            .or_default()
            .insert(target.id.clone(), target);
    }

    pub fn targets(&self, pointers: &[&'a Declaration]) -> Vec<&'a Declaration> {
        let mut unique = BTreeMap::<String, &'a Declaration>::new();
        for pointer in pointers {
            if let Some(targets) = self.by_pointer.get(&pointer.id) {
                for (id, target) in targets {
                    unique.insert(id.clone(), *target);
                }
            }
        }
        unique.into_values().collect()
    }
}

pub fn resolve_pointer_declarations<'a>(
    index: &'a DeclarationIndex<'a>,
    observation: &CallObservation,
) -> Vec<&'a Declaration> {
    if !observation.member {
        let local_key = container_key(&observation.source_id, &observation.candidate);
        let local = pointer_values(index.by_container_name.get(&local_key));
        if !local.is_empty() {
            return local;
        }
        let path_key = format!("{}\0{}", observation.path, observation.candidate);
        return pointer_values(index.by_path_name.get(&path_key))
            .into_iter()
            .filter(|value| value.kind != "field")
            .collect();
    }

    pointer_values(
        index
            .by_name
            .get(&last_qualified_part(&observation.candidate)),
    )
    .into_iter()
    .filter(|value| value.kind == "field")
    .collect()
}

fn pointer_values<'a>(values: Option<&Vec<&'a Declaration>>) -> Vec<&'a Declaration> {
    let mut unique = BTreeMap::<String, &'a Declaration>::new();
    for value in values.into_iter().flatten() {
        if is_function_pointer(value) {
            unique.insert(value.id.clone(), *value);
        }
    }
    unique.into_values().collect()
}

fn direct_callable_targets<'a>(
    index: &'a DeclarationIndex<'a>,
    observation: &CallObservation,
) -> Vec<&'a Declaration> {
    index.resolve(
        &observation.candidate,
        &observation.source_scope,
        &observation.path,
        |value| value.callable,
    )
}

fn resolve_callable_reference<'a>(
    index: &'a DeclarationIndex<'a>,
    candidate: &str,
    scope: &str,
    path: &str,
) -> Vec<&'a Declaration> {
    index.resolve(candidate, scope, path, |value| value.callable)
}
