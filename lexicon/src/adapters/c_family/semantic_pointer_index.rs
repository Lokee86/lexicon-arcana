use std::collections::{BTreeMap, BTreeSet};

use super::{
    model::{RepositoryModel, SemanticCallResolution},
    pointer_aliases::is_function_pointer,
    resolution::DeclarationIndex,
};

pub(super) struct SemanticPointerIndex {
    by_pointer: BTreeMap<String, BTreeSet<String>>,
}

impl SemanticPointerIndex {
    pub(super) fn build(model: &RepositoryModel, declarations: &DeclarationIndex<'_>) -> Self {
        let mut result = Self {
            by_pointer: BTreeMap::new(),
        };

        for file in &model.files {
            for binding in &file.semantic_pointer_bindings {
                if binding.pointer_id.is_empty() || binding.target_id.is_empty() {
                    continue;
                }
                result
                    .by_pointer
                    .entry(binding.pointer_id.clone())
                    .or_default()
                    .insert(binding.target_id.clone());
            }
        }

        loop {
            let mut changed = false;
            for file in &model.files {
                for call in &file.semantic_calls {
                    if call.resolution != SemanticCallResolution::Resolved
                        || call.target_id.is_empty()
                    {
                        continue;
                    }
                    let Some(parameters) = declarations.by_callable_parameters.get(&call.target_id)
                    else {
                        continue;
                    };
                    for (index, argument) in call.arguments.iter().enumerate() {
                        let Some(parameter) = parameters.iter().find(|parameter| {
                            parameter
                                .attributes
                                .get("index")
                                .and_then(|value| value.as_u64())
                                == Some(index as u64)
                        }) else {
                            continue;
                        };
                        if !is_function_pointer(parameter) {
                            continue;
                        }

                        let mut targets = BTreeSet::new();
                        if !argument.callable_id.is_empty() {
                            targets.insert(argument.callable_id.clone());
                        }
                        if !argument.value_id.is_empty()
                            && let Some(values) = result.by_pointer.get(&argument.value_id)
                        {
                            targets.extend(values.iter().cloned());
                        }
                        if targets.is_empty() {
                            continue;
                        }

                        let entry = result.by_pointer.entry(parameter.id.clone()).or_default();
                        let before = entry.len();
                        entry.extend(targets);
                        changed |= entry.len() != before;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        result
    }

    pub(super) fn targets(&self, pointer_id: &str) -> Vec<String> {
        self.by_pointer
            .get(pointer_id)
            .map(|values| values.iter().cloned().collect())
            .unwrap_or_default()
    }
}
