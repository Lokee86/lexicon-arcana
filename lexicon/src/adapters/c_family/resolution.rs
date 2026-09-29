use super::model::{Declaration, RepositoryModel};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct DeclarationIndex<'a> {
    pub(super) by_callable_parameters: BTreeMap<String, Vec<&'a Declaration>>,
}

impl<'a> DeclarationIndex<'a> {
    pub fn new(model: &'a RepositoryModel) -> Self {
        let mut by_callable_parameters = BTreeMap::<String, Vec<&Declaration>>::new();
        for file in &model.files {
            for declaration in &file.declarations {
                if declaration.kind == "parameter" {
                    by_callable_parameters
                        .entry(declaration.container_id.clone())
                        .or_default()
                        .push(declaration);
                }
            }
        }
        for values in by_callable_parameters.values_mut() {
            values.sort_by_key(|value| {
                value
                    .attributes
                    .get("index")
                    .and_then(|index| index.as_u64())
                    .unwrap_or(u64::MAX)
            });
        }
        Self {
            by_callable_parameters,
        }
    }
}
