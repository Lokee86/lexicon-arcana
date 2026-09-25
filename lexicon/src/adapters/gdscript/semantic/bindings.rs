use std::collections::{BTreeMap, BTreeSet};

use super::{OwnerSet, SemanticModel};
use crate::adapters::gdscript::facts::{Facts, normalize_path};
use crate::adapters::gdscript::model::AnalysisContext;
use crate::adapters::gdscript::parser::is_builtin;

impl SemanticModel {
    pub fn type_owners(&self, facts: &Facts, path: &str, type_name: &str) -> OwnerSet {
        let mut name = type_name.trim();
        if name.is_empty() || is_builtin_type(name) {
            return OwnerSet::new();
        }
        if let Some(index) = name.find('[') {
            name = name[..index].trim();
        }
        if let Some(index) = name.rfind('.') {
            let (prefix, nested) = (name[..index].trim(), name[index + 1..].trim());
            if let Some(owners) = self.preload_alias_owners(facts, path, prefix) {
                let targets = unique(
                    owners
                        .iter()
                        .flat_map(|owner| {
                            facts
                                .type_by_owner_id
                                .get(owner)
                                .and_then(|values| values.get(nested))
                                .cloned()
                                .unwrap_or_default()
                        })
                        .collect(),
                );
                if targets.len() == 1 {
                    return OwnerSet::from([targets[0].clone()]);
                }
            }
            name = nested;
        }
        resolve_class_id(facts, path, name)
            .map(|id| OwnerSet::from([id]))
            .unwrap_or_default()
    }

    pub fn class_owners(&self, facts: &Facts, path: &str, name: &str) -> (OwnerSet, bool) {
        let path = normalize_path(path);
        if let Some(ids) = facts
            .class_by_file_and_name
            .get(&path)
            .and_then(|values| values.get(name))
        {
            return if ids.len() == 1 {
                (OwnerSet::from([ids[0].clone()]), false)
            } else {
                (OwnerSet::new(), true)
            };
        }
        let ids = facts.class_by_name.get(name).cloned().unwrap_or_default();
        if ids.len() == 1 {
            (OwnerSet::from([ids[0].clone()]), false)
        } else {
            (OwnerSet::new(), ids.len() > 1)
        }
    }

    pub fn preload_alias_owners(&self, facts: &Facts, path: &str, name: &str) -> Option<OwnerSet> {
        let paths = facts
            .preload_alias_by_file_and_name
            .get(&normalize_path(path))
            .and_then(|values| values.get(name))?;
        let mut owners = OwnerSet::new();
        for target_path in paths {
            if let Some(candidates) = facts.script_owner_candidates_by_path.get(target_path) {
                owners.extend(candidates.iter().cloned());
            } else if let Some(owner) = facts.script_owner_by_path.get(target_path) {
                owners.insert(owner.clone());
            }
        }
        Some(owners)
    }

    pub fn preload_alias_reason(&self, facts: &Facts, path: &str, name: &str) -> &'static str {
        let paths = facts
            .preload_alias_by_file_and_name
            .get(&normalize_path(path))
            .and_then(|values| values.get(name))
            .cloned()
            .unwrap_or_default();
        if paths.iter().any(|path| {
            std::path::Path::new(path)
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("gd"))
        }) {
            "missing-target"
        } else {
            "external-target"
        }
    }

    pub fn has_external_parent(
        &self,
        facts: &Facts,
        owner: &str,
        seen: &mut BTreeSet<String>,
    ) -> bool {
        if owner.is_empty() || !seen.insert(owner.into()) {
            return false;
        }
        if facts.external_parent_by_owner_id.contains(owner) {
            return true;
        }
        facts.parent_by_owner_id.get(owner).is_some_and(|parents| {
            parents
                .iter()
                .any(|parent| self.has_external_parent(facts, parent, seen))
        })
    }

    pub fn binding_is_builtin(
        &self,
        facts: &Facts,
        files: &[crate::adapters::gdscript::model::ParsedFile],
        context: &AnalysisContext,
        name: &str,
    ) -> bool {
        if !context.function_id.is_empty()
            && let Some(declaration) = facts.declaration_by_id.get(&context.function_id)
            && declaration
                .parameter_types
                .get(name)
                .is_some_and(|value| is_builtin_type(value))
        {
            return true;
        }
        files[context.file_index]
            .declarations
            .iter()
            .any(|declaration| {
                declaration.name == name
                    && !declaration.type_name.is_empty()
                    && is_builtin_type(&declaration.type_name)
            })
    }

    pub fn add_type_alias(&mut self, path: &str, name: &str, values: OwnerSet) -> bool {
        if path.is_empty() || name.is_empty() || values.is_empty() {
            return false;
        }
        merge_binding(
            self.type_aliases.entry(normalize_path(path)).or_default(),
            name,
            values,
        )
    }

    pub fn type_alias_owners(&self, path: &str, name: &str) -> OwnerSet {
        self.type_aliases
            .get(&normalize_path(path))
            .and_then(|values| values.get(name))
            .cloned()
            .unwrap_or_default()
    }

    pub fn binding_declared(&self, facts: &Facts, context: &AnalysisContext, name: &str) -> bool {
        (!context.function_id.is_empty() && self.local_declared(facts, &context.function_id, name))
            || self.member_declared(facts, &context.owner_id, name, &mut BTreeSet::new())
    }

    pub fn add_member(&mut self, owner: &str, name: &str, values: OwnerSet) -> bool {
        if owner.is_empty() || name.is_empty() || values.is_empty() {
            return false;
        }
        merge_binding(self.members.entry(owner.into()).or_default(), name, values)
    }

    pub fn add_local(&mut self, function: &str, name: &str, values: OwnerSet) -> bool {
        if function.is_empty() || name.is_empty() || values.is_empty() {
            return false;
        }
        merge_binding(
            self.locals.entry(function.into()).or_default(),
            name,
            values,
        )
    }

    pub fn add_return(&mut self, function: &str, values: OwnerSet) -> bool {
        if function.is_empty() || values.is_empty() {
            return false;
        }
        merge_set(self.returns.entry(function.into()).or_default(), values)
    }

    pub fn local_declared(&self, facts: &Facts, function: &str, name: &str) -> bool {
        facts
            .declared_local_by_function
            .get(function)
            .is_some_and(|values| values.contains(name))
    }

    pub fn member_declared(
        &self,
        facts: &Facts,
        owner: &str,
        name: &str,
        seen: &mut BTreeSet<String>,
    ) -> bool {
        if owner.is_empty() || !seen.insert(owner.into()) {
            return false;
        }
        if facts
            .declared_member_by_owner
            .get(owner)
            .is_some_and(|values| values.contains(name))
        {
            return true;
        }
        facts.parent_by_owner_id.get(owner).is_some_and(|parents| {
            parents
                .iter()
                .any(|parent| self.member_declared(facts, parent, name, seen))
        })
    }
}

pub fn is_callable_invocation(name: &str) -> bool {
    matches!(name, "call" | "callv" | "call_deferred")
}

pub fn is_builtin_type(value: &str) -> bool {
    let name = value
        .trim()
        .split_once('[')
        .map_or(value.trim(), |(name, _)| name.trim());
    matches!(
        name,
        "void"
            | "Variant"
            | "bool"
            | "int"
            | "float"
            | "String"
            | "StringName"
            | "NodePath"
            | "Array"
            | "Dictionary"
            | "PackedByteArray"
            | "PackedInt32Array"
            | "PackedInt64Array"
            | "PackedFloat32Array"
            | "PackedFloat64Array"
            | "PackedStringArray"
            | "Vector2"
            | "Vector2i"
            | "Vector3"
            | "Vector3i"
            | "Vector4"
            | "Vector4i"
            | "Rect2"
            | "Rect2i"
            | "Transform2D"
            | "Transform3D"
            | "Basis"
            | "Quaternion"
            | "Plane"
            | "Projection"
            | "AABB"
            | "Color"
            | "RID"
            | "Callable"
            | "Signal"
    ) || is_builtin(name)
}

fn resolve_class_id(facts: &Facts, source_path: &str, name: &str) -> Option<String> {
    if let Some(ids) = facts
        .class_by_file_and_name
        .get(&normalize_path(source_path))
        .and_then(|values| values.get(name))
    {
        if ids.len() == 1 {
            return Some(ids[0].clone());
        }
        if ids.len() > 1 {
            return None;
        }
    }
    let ids = facts.class_by_name.get(name)?;
    (ids.len() == 1).then(|| ids[0].clone())
}

pub fn merge_set(target: &mut OwnerSet, values: OwnerSet) -> bool {
    let before = target.len();
    target.extend(values);
    target.len() != before
}

pub fn merge_binding(
    bindings: &mut BTreeMap<String, OwnerSet>,
    name: &str,
    values: OwnerSet,
) -> bool {
    if values.is_empty() {
        return false;
    }
    merge_set(bindings.entry(name.into()).or_default(), values)
}

pub fn union(sets: impl IntoIterator<Item = OwnerSet>) -> OwnerSet {
    sets.into_iter().flatten().collect()
}

pub fn unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}
