use std::collections::{BTreeMap, BTreeSet};

use super::model::{Declaration, ParameterDecl, ParsedFile};
use super::tokens::contains;

#[derive(Default)]
pub struct RuntimeIndex {
    pub callables: BTreeMap<String, RuntimeCallable>,
    pub callables_by_key: BTreeMap<String, Vec<String>>,
    pub constructors: BTreeMap<String, Vec<String>>,
    pub direct_companions_by_owner: BTreeMap<String, Vec<String>>,
    pub extensions_by_qn: BTreeMap<String, Vec<String>>,
    pub ordinary_members_by_key: BTreeMap<String, RuntimeAcceptedArities>,
    pub properties_by_key: BTreeMap<String, Vec<RuntimeProperty>>,
    pub types_by_id: BTreeMap<String, RuntimeType>,
    pub types_by_qn: BTreeMap<String, Vec<String>>,
}

#[derive(Default)]
pub struct RuntimeAcceptedArities {
    exact: BTreeSet<usize>,
    variadic_from: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct RuntimeCallable {
    pub declaration: Declaration,
    pub file: ParsedFile,
    pub id: String,
    pub kind: String,
    pub owner_kind: String,
    pub owner_qn: String,
    pub parameters: BTreeMap<String, Vec<String>>,
    pub signature: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeProperty {
    pub declaration: Declaration,
    pub file: ParsedFile,
    pub id: String,
    pub owner_qn: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeType {
    pub declaration: Declaration,
    pub file: ParsedFile,
    pub form: String,
    pub id: String,
    pub owner_qn: String,
    pub qualified: String,
}

impl super::state::AnalysisState {
    #[allow(clippy::too_many_arguments)]
    pub fn index_runtime_declaration(
        &mut self,
        file: &ParsedFile,
        declaration: &Declaration,
        id: &str,
        kind: &str,
        owner_qn: &str,
        owner_kind: &str,
        qualified_base: &str,
    ) {
        match kind {
            "type" | "interface" => {
                let target = RuntimeType {
                    declaration: declaration.clone(),
                    file: file.clone(),
                    form: declaration.form.clone(),
                    id: id.into(),
                    owner_qn: owner_qn.into(),
                    qualified: qualified_base.into(),
                };
                self.runtime.types_by_id.insert(id.into(), target.clone());
                push_unique(
                    self.runtime
                        .types_by_qn
                        .entry(qualified_base.into())
                        .or_default(),
                    id.into(),
                );
                if target.form == "companion_object" {
                    push_unique(
                        self.runtime
                            .direct_companions_by_owner
                            .entry(owner_qn.into())
                            .or_default(),
                        id.into(),
                    );
                }
            }
            "field" => {
                let property = RuntimeProperty {
                    declaration: declaration.clone(),
                    file: file.clone(),
                    id: id.into(),
                    owner_qn: owner_qn.into(),
                };
                self.runtime
                    .properties_by_key
                    .entry(runtime_member_key(owner_qn, &declaration.name))
                    .or_default()
                    .push(property);
            }
            "function" | "method" | "constructor" => {
                let callable = RuntimeCallable {
                    declaration: declaration.clone(),
                    file: file.clone(),
                    id: id.into(),
                    kind: kind.into(),
                    owner_kind: owner_kind.into(),
                    owner_qn: owner_qn.into(),
                    parameters: BTreeMap::new(),
                    signature: normalized_parameter_signature(&declaration.parameters),
                };
                self.runtime.callables.insert(id.into(), callable);
                if declaration.receiver.is_empty() {
                    self.runtime.index_ordinary_member(owner_qn, declaration);
                }
                if kind == "constructor" {
                    push_unique(
                        self.runtime
                            .constructors
                            .entry(runtime_arity_key(owner_qn, declaration.parameters.len()))
                            .or_default(),
                        id.into(),
                    );
                } else {
                    push_unique(
                        self.runtime
                            .callables_by_key
                            .entry(runtime_callable_key(
                                owner_qn,
                                &declaration.name,
                                declaration.parameters.len(),
                            ))
                            .or_default(),
                        id.into(),
                    );
                    if !declaration.receiver.is_empty() {
                        push_unique(
                            self.runtime
                                .extensions_by_qn
                                .entry(qualified_base.into())
                                .or_default(),
                            id.into(),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    pub fn index_runtime_parameter(&mut self, callable: &str, name: &str, parameter: &str) {
        if let Some(callable) = self.runtime.callables.get_mut(callable) {
            push_unique(
                callable.parameters.entry(name.into()).or_default(),
                parameter.into(),
            );
        }
    }
}

impl RuntimeIndex {
    fn index_ordinary_member(&mut self, owner: &str, declaration: &Declaration) {
        let arities = self
            .ordinary_members_by_key
            .entry(runtime_member_key(owner, &declaration.name))
            .or_default();
        for arity in 0..=declaration.parameters.len() {
            if callable_accepts_arity(declaration, arity) {
                arities.exact.insert(arity);
            }
        }
        let variadic_from = declaration.parameters.len() + 1;
        if callable_accepts_arity(declaration, variadic_from)
            && arities
                .variadic_from
                .is_none_or(|current| variadic_from < current)
        {
            arities.variadic_from = Some(variadic_from);
        }
    }

    pub fn has_ordinary_member(&self, owner: &str, name: &str, arity: usize) -> bool {
        let Some(arities) = self
            .ordinary_members_by_key
            .get(&runtime_member_key(owner, name))
        else {
            return false;
        };
        arities.exact.contains(&arity)
            || arities
                .variadic_from
                .is_some_and(|minimum| arity >= minimum)
    }

    pub fn callable_ids(&self) -> Vec<String> {
        self.callables.keys().cloned().collect()
    }
}

pub fn runtime_callable_key(owner: &str, name: &str, arity: usize) -> String {
    format!("{owner}\0{name}\0{arity}")
}

pub fn runtime_arity_key(owner: &str, arity: usize) -> String {
    format!("{owner}\0{arity}")
}

pub fn runtime_member_key(owner: &str, name: &str) -> String {
    format!("{owner}\0{name}")
}

pub fn normalize_runtime_type(value: &str) -> String {
    value.split_whitespace().collect::<String>()
}

pub fn normalized_receiver(declaration: &Declaration) -> String {
    normalize_runtime_type(&declaration.receiver)
}

fn normalized_parameter_signature(parameters: &[ParameterDecl]) -> String {
    parameters
        .iter()
        .map(|parameter| normalize_runtime_type(&parameter.type_name))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn callable_accepts_arity(declaration: &Declaration, arity: usize) -> bool {
    let parameters = &declaration.parameters;
    let mut vararg = None;
    for (index, parameter) in parameters.iter().enumerate() {
        if contains(&parameter.modifiers, "vararg") {
            if vararg.is_some() || index != parameters.len().saturating_sub(1) {
                return arity == parameters.len();
            }
            vararg = Some(index);
        }
    }
    if let Some(vararg) = vararg
        && arity > vararg
    {
        return true;
    }
    if arity > parameters.len() {
        return false;
    }
    let end = vararg.unwrap_or(parameters.len());
    for parameter in parameters.iter().take(end).skip(arity) {
        if !parameter.has_default {
            return false;
        }
    }
    arity <= end || vararg.is_some()
}

pub fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}
