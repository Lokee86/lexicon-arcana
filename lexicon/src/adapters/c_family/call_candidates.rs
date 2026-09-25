use super::{
    model::{CallObservation, Declaration},
    receiver_resolution::{container_key, direct_receiver_type_id},
    resolution::{DeclarationIndex, strip_template_arguments},
    syntax::{last_qualified_part, normalize_qualified},
};

#[derive(Debug)]
pub struct CallCandidateResolution<'a> {
    pub candidates: Vec<&'a Declaration>,
    pub evidence: Vec<String>,
}

impl<'a> CallCandidateResolution<'a> {
    pub fn prune(mut self, argument_count: usize) -> Self {
        let pruned = prune_callable_candidates(&self.candidates, argument_count);
        if pruned.len() != self.candidates.len() {
            self.candidates = pruned;
            self.evidence.push("arity-pruning".into());
        }
        self
    }
}

pub fn resolve<'a>(
    index: &'a DeclarationIndex<'a>,
    observation: &CallObservation,
) -> CallCandidateResolution<'a> {
    let accept = |value: &Declaration| value.callable;
    if let Some(qualifier) = explicit_qualifier(&observation.candidate) {
        let candidates = index.resolve(
            &observation.candidate,
            &observation.source_scope,
            &observation.path,
            accept,
        );
        let mut evidence = vec!["explicit-qualification".into()];
        let types = direct_qualified_types(index, &qualifier, &observation.path);
        if !types.is_empty() {
            let fallback = if candidates.is_empty() {
                index.resolve(
                    &last_qualified_part(&observation.candidate),
                    &observation.source_scope,
                    &observation.path,
                    accept,
                )
            } else {
                candidates
            };
            let owned = owned_callables(
                index,
                &types,
                &last_qualified_part(&observation.candidate),
                &observation.path,
            );
            if !owned.is_empty() {
                evidence.push("enclosing-type-ownership".into());
                return CallCandidateResolution {
                    candidates: owned,
                    evidence,
                };
            }
            return CallCandidateResolution {
                candidates: fallback,
                evidence,
            };
        }

        let free = candidates
            .iter()
            .copied()
            .filter(|value| !class_owned_callable(index, value))
            .collect::<Vec<_>>();
        return CallCandidateResolution {
            candidates: if free.is_empty() { candidates } else { free },
            evidence,
        };
    }

    let receiver_type_id = direct_receiver_type_id(index, observation);
    if !receiver_type_id.is_empty() {
        let key = container_key(
            &receiver_type_id,
            &last_qualified_part(&observation.candidate),
        );
        if let Some(owned) =
            index.select(index.by_container_name.get(&key), &observation.path, accept)
            && !owned.is_empty()
        {
            return CallCandidateResolution {
                candidates: owned,
                evidence: vec!["direct-receiver-type".into()],
            };
        }
    }

    if !observation.receiver_type_id.is_empty() {
        let candidates = index.resolve(
            &observation.candidate,
            &observation.source_scope,
            &observation.path,
            accept,
        );
        let key = container_key(
            &observation.receiver_type_id,
            &last_qualified_part(&observation.candidate),
        );
        if let Some(owned) =
            index.select(index.by_container_name.get(&key), &observation.path, accept)
            && !owned.is_empty()
        {
            return CallCandidateResolution {
                candidates: owned,
                evidence: vec!["enclosing-type-ownership".into()],
            };
        }
        return CallCandidateResolution {
            candidates,
            evidence: vec!["direct-scoped-name".into()],
        };
    }

    CallCandidateResolution {
        candidates: index.resolve(
            &observation.candidate,
            &observation.source_scope,
            &observation.path,
            accept,
        ),
        evidence: vec!["direct-scoped-name".into()],
    }
}

pub fn explicit_qualifier(candidate: &str) -> Option<String> {
    let candidate = normalize_qualified(candidate);
    let separator = candidate.rfind("::")?;
    (separator > 0 && separator + 2 < candidate.len()).then(|| candidate[..separator].into())
}

pub fn direct_qualified_types<'a>(
    index: &'a DeclarationIndex<'a>,
    qualifier: &str,
    path: &str,
) -> Vec<&'a Declaration> {
    let qualified = strip_template_arguments(&normalize_qualified(qualifier));
    index
        .select(index.by_qualified.get(&qualified), path, |value| {
            value.kind == "type"
        })
        .unwrap_or_default()
}

pub fn has_callable(index: &DeclarationIndex<'_>, candidate: &str) -> bool {
    index
        .by_name
        .get(&last_qualified_part(candidate))
        .is_some_and(|values| values.iter().any(|value| value.callable))
}

fn owned_callables<'a>(
    index: &'a DeclarationIndex<'a>,
    types: &[&Declaration],
    name: &str,
    path: &str,
) -> Vec<&'a Declaration> {
    let mut values = Vec::new();
    for typ in types {
        let key = container_key(&typ.id, name);
        values.extend(
            index
                .select(index.by_container_name.get(&key), path, |value| {
                    value.callable
                })
                .unwrap_or_default(),
        );
    }
    values.sort_by(|left, right| left.id.cmp(&right.id));
    values.dedup_by(|left, right| left.id == right.id);
    values
}

fn class_owned_callable(index: &DeclarationIndex<'_>, value: &Declaration) -> bool {
    if matches!(value.kind.as_str(), "method" | "constructor") || !value.parent_type_id.is_empty() {
        return true;
    }
    explicit_qualifier(&value.qualified_name)
        .is_some_and(|qualifier| !direct_qualified_types(index, &qualifier, &value.path).is_empty())
}

fn prune_callable_candidates<'a>(
    candidates: &[&'a Declaration],
    argument_count: usize,
) -> Vec<&'a Declaration> {
    if candidates.len() < 2 {
        return candidates.to_vec();
    }
    let compatible = candidates
        .iter()
        .copied()
        .filter(|value| {
            value.callable_shape.is_none_or(|shape| {
                argument_count >= shape.minimum
                    && (shape.variadic
                        || shape
                            .maximum
                            .is_none_or(|maximum| argument_count <= maximum))
            })
        })
        .collect::<Vec<_>>();
    if compatible.is_empty() {
        candidates.to_vec()
    } else {
        compatible
    }
}
