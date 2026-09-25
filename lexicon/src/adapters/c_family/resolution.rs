use super::{
    model::{Declaration, RepositoryModel},
    syntax::{last_qualified_part, normalize_qualified},
    visibility::VisibilityIndex,
};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct DeclarationIndex<'a> {
    by_qualified: BTreeMap<String, Vec<&'a Declaration>>,
    by_name: BTreeMap<String, Vec<&'a Declaration>>,
    visibility: &'a VisibilityIndex,
}

impl<'a> DeclarationIndex<'a> {
    pub fn new(model: &'a RepositoryModel) -> Self {
        let mut by_qualified = BTreeMap::<String, Vec<&Declaration>>::new();
        let mut by_name = BTreeMap::<String, Vec<&Declaration>>::new();
        for file in &model.files {
            for declaration in &file.declarations {
                by_qualified
                    .entry(normalize_qualified(&declaration.qualified_name))
                    .or_default()
                    .push(declaration);
                by_name
                    .entry(declaration.name.clone())
                    .or_default()
                    .push(declaration);
            }
        }
        for values in by_qualified.values_mut() {
            values.sort_by(|left, right| left.id.cmp(&right.id));
        }
        for values in by_name.values_mut() {
            values.sort_by(|left, right| left.id.cmp(&right.id));
        }
        Self {
            by_qualified,
            by_name,
            visibility: &model.visibility,
        }
    }

    pub fn resolve<F>(
        &self,
        candidate: &str,
        scope: &str,
        path: &str,
        accept: F,
    ) -> Vec<&'a Declaration>
    where
        F: Fn(&Declaration) -> bool + Copy,
    {
        let candidate = strip_template_arguments(&normalize_qualified(candidate));
        if candidate.is_empty() {
            return Vec::new();
        }

        if candidate.contains("::")
            && let Some(matches) = self.select(self.by_qualified.get(&candidate), path, accept)
        {
            return matches;
        }

        let mut current = normalize_qualified(scope);
        while !current.is_empty() {
            let qualified = format!("{current}::{candidate}");
            if let Some(matches) = self.select(self.by_qualified.get(&qualified), path, accept) {
                return matches;
            }
            current = parent_scope(&current).into();
        }

        if let Some(matches) = self.select(self.by_qualified.get(&candidate), path, accept) {
            return matches;
        }

        let name = last_qualified_part(&candidate);
        self.select(self.by_name.get(&name), path, accept)
            .unwrap_or_default()
    }

    fn select<F>(
        &self,
        values: Option<&Vec<&'a Declaration>>,
        path: &str,
        accept: F,
    ) -> Option<Vec<&'a Declaration>>
    where
        F: Fn(&Declaration) -> bool + Copy,
    {
        let visible = unique_sorted(values?, |value| {
            accept(value) && self.visibility.declaration_visible(path, value)
        });
        if visible.is_empty() {
            return None;
        }

        let same_file = visible
            .iter()
            .copied()
            .filter(|value| value.path == path)
            .collect::<Vec<_>>();
        if !same_file.is_empty() {
            return Some(prefer_definitions(same_file));
        }
        Some(prefer_definitions(visible))
    }
}

pub fn resolution_reason(candidates: &[&Declaration]) -> &'static str {
    if candidates.len() > 1 {
        "ambiguous-target"
    } else {
        "missing-target"
    }
}

fn unique_sorted<'a, F>(values: &[&'a Declaration], accept: F) -> Vec<&'a Declaration>
where
    F: Fn(&Declaration) -> bool,
{
    let mut unique = BTreeMap::<&str, &Declaration>::new();
    for value in values {
        if accept(value) {
            // Matching facts-v1 identities intentionally collapse here. Later
            // declarations replace earlier ones, so a definition can supersede
            // a same-signature prototype exactly as in the Go oracle.
            unique.insert(value.id.as_str(), *value);
        }
    }
    unique.into_values().collect()
}

fn prefer_definitions(values: Vec<&Declaration>) -> Vec<&Declaration> {
    let definitions = values
        .iter()
        .copied()
        .filter(|value| value.definition)
        .collect::<Vec<_>>();
    if definitions.is_empty() {
        values
    } else {
        definitions
    }
}

fn parent_scope(scope: &str) -> &str {
    scope.rsplit_once("::").map_or("", |(parent, _)| parent)
}

fn strip_template_arguments(value: &str) -> String {
    value
        .split_once('<')
        .map_or(value, |(prefix, _)| prefix)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{parent_scope, strip_template_arguments};

    #[test]
    fn helper_normalization_matches_oracle_policy() {
        assert_eq!(parent_scope("demo::nested::Type"), "demo::nested");
        assert_eq!(parent_scope("demo"), "");
        assert_eq!(strip_template_arguments("Base<int>"), "Base");
        assert_eq!(strip_template_arguments("demo::Base<T>"), "demo::Base");
    }
}
