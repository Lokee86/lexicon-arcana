use super::model::ParsedFile;
use super::runtime::{RuntimeCallable, RuntimeType, normalize_runtime_type, runtime_member_key};
use super::runtime_locals;
use super::runtime_resolution::runtime_package;
use super::runtime_tokens::RuntimeInvocation;
use super::state::AnalysisState;

#[derive(Clone)]
pub struct RuntimeDeclaredType {
    pub file: ParsedFile,
    pub lexical_owner: String,
    pub spelling: String,
}

impl AnalysisState {
    pub fn receiver_evidence(
        &self,
        callable: &RuntimeCallable,
        invocation: &RuntimeInvocation,
    ) -> Option<RuntimeDeclaredType> {
        let name = &invocation.qualifier;
        if name.is_empty() || name.contains('.') {
            return None;
        }
        if let Some(spelling) =
            runtime_locals::declared_type(callable, name, invocation.callee_start)
        {
            return Some(RuntimeDeclaredType {
                file: callable.file.clone(),
                lexical_owner: callable.owner_qn.clone(),
                spelling,
            });
        }

        let parameter_types = callable
            .declaration
            .parameters
            .iter()
            .filter(|parameter| parameter.name == *name)
            .map(|parameter| parameter.type_name.clone())
            .collect::<Vec<_>>();
        if parameter_types.len() == 1 {
            return Some(RuntimeDeclaredType {
                file: callable.file.clone(),
                lexical_owner: callable.owner_qn.clone(),
                spelling: parameter_types[0].clone(),
            });
        }
        if parameter_types.len() > 1 {
            return Some(RuntimeDeclaredType {
                file: callable.file.clone(),
                lexical_owner: callable.owner_qn.clone(),
                spelling: String::new(),
            });
        }

        let mut owners = vec![callable.owner_qn.clone()];
        if matches!(callable.owner_kind.as_str(), "type" | "interface") {
            owners.push(runtime_package(&callable.file));
        }
        for owner in owners {
            if let Some(evidence) = self.property_declared_type(&owner, name) {
                return Some(evidence);
            }
        }
        None
    }

    pub fn property_declared_type(&self, owner: &str, name: &str) -> Option<RuntimeDeclaredType> {
        let evidence = self
            .runtime
            .properties_by_key
            .get(&runtime_member_key(owner, name))
            .into_iter()
            .flatten()
            .filter(|property| {
                property.declaration.receiver.is_empty() && !property.declaration.delegated
            })
            .map(|property| RuntimeDeclaredType {
                file: property.file.clone(),
                lexical_owner: property.owner_qn.clone(),
                spelling: property.declaration.type_name.clone(),
            })
            .collect::<Vec<_>>();
        (evidence.len() == 1).then(|| evidence[0].clone())
    }

    pub fn resolve_declared_type(
        &self,
        evidence: &RuntimeDeclaredType,
    ) -> (Option<RuntimeType>, bool, String) {
        let Some((name, nullable)) = simple_type_name(&evidence.spelling) else {
            return (None, false, "unsupported-form".into());
        };
        let (targets, reason) =
            self.resolve_runtime_types(&evidence.file, &evidence.lexical_owner, &name);
        if targets.len() != 1 {
            return (None, false, reason);
        }
        (Some(targets[0].clone()), nullable, String::new())
    }
}

fn simple_type_name(spelling: &str) -> Option<(String, bool)> {
    let mut name = normalize_runtime_type(spelling);
    let nullable = name.ends_with('?');
    name = name.trim_end_matches('?').to_owned();
    if name.is_empty() || name.starts_with('.') || name.ends_with('.') || name.contains("..") {
        return None;
    }
    if name.chars().any(|current| {
        current != '.'
            && current != '_'
            && current != '\x60'
            && !current.is_alphanumeric()
            && !current.is_alphabetic()
    }) {
        return None;
    }
    let mut parts = Vec::new();
    for part in name.split('.') {
        let part = part.trim_matches('\x60');
        if part.is_empty() {
            return None;
        }
        parts.push(part);
    }
    Some((parts.join("."), nullable))
}
