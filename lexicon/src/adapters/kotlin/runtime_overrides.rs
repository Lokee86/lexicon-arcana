use super::runtime::{RuntimeType, normalized_receiver, runtime_callable_key};
use super::runtime_resolution::unique_types;
use super::state::AnalysisState;

impl AnalysisState {
    pub fn emit_overrides(&mut self) {
        let callables = self.runtime.callables.values().cloned().collect::<Vec<_>>();
        for callable in callables {
            if callable.kind != "method" {
                continue;
            }
            for supertype in self.direct_runtime_supertypes(&callable.owner_qn) {
                let candidates = self
                    .runtime
                    .callables_by_key
                    .get(&runtime_callable_key(
                        &supertype.qualified,
                        &callable.declaration.name,
                        callable.declaration.parameters.len(),
                    ))
                    .into_iter()
                    .flatten()
                    .filter_map(|id| self.runtime.callables.get(id))
                    .filter(|candidate| {
                        candidate.kind == "method"
                            && candidate.signature == callable.signature
                            && normalized_receiver(&candidate.declaration)
                                == normalized_receiver(&callable.declaration)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let candidates = super::runtime_resolution::unique_callables(candidates);
                if candidates.len() == 1 {
                    self.facts.add_edge(
                        &callable.id,
                        &candidates[0].id,
                        "overrides",
                        Some(&callable.file.path),
                        Some(callable.declaration.span.clone()),
                        None,
                    );
                }
            }
        }
    }

    pub fn direct_runtime_supertypes(&self, owner_qn: &str) -> Vec<RuntimeType> {
        let owners = self
            .runtime
            .types_by_qn
            .get(owner_qn)
            .into_iter()
            .flatten()
            .filter_map(|id| self.runtime.types_by_id.get(id).cloned())
            .collect::<Vec<_>>();
        let owners = unique_types(owners);
        if owners.len() != 1 {
            return Vec::new();
        }
        let owner = &owners[0];
        let mut targets = Vec::new();
        for supertype in &owner.declaration.supertypes {
            let (resolved, _) =
                self.resolve_runtime_types(&owner.file, &owner.owner_qn, &supertype.target_name);
            if resolved.len() == 1 {
                targets.push(resolved[0].clone());
            }
        }
        unique_types(targets)
    }
}
