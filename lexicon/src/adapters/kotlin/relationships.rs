use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::SourceSpan;

use super::facts::Facts;
use super::model::{ParsedFile, SupertypeDecl};

#[derive(Clone)]
pub struct RelationshipTarget {
    pub form: String,
    pub id: String,
    pub kind: String,
}

#[derive(Clone)]
pub struct PendingRelationship {
    pub attributes: Option<Value>,
    pub expression: String,
    pub file: ParsedFile,
    pub kind: String,
    pub lexical_owner: String,
    pub span: SourceSpan,
    pub source: String,
    pub target_name: String,
}

pub fn annotation_reference(annotation: &str) -> &str {
    annotation
        .rsplit_once(':')
        .map_or(annotation, |(_, value)| value)
}

pub fn imported_alias_names(file: &ParsedFile, name: &str) -> (Vec<String>, bool) {
    let (first, suffix) = name
        .split_once('.')
        .map_or((name, ""), |(first, _)| (first, &name[first.len()..]));
    let names = file
        .imports
        .iter()
        .filter(|imported| imported.alias == first)
        .map(|imported| format!("{}{}", imported.path, suffix))
        .collect::<Vec<_>>();
    let bound = !names.is_empty();
    (names, bound)
}

pub fn explicit_import_names(file: &ParsedFile, name: &str) -> (Vec<String>, bool) {
    let (first, suffix) = name
        .split_once('.')
        .map_or((name, ""), |(first, _)| (first, &name[first.len()..]));
    let names = file
        .imports
        .iter()
        .filter(|imported| {
            !imported.wildcard && imported.alias.is_empty() && simple_name(&imported.path) == first
        })
        .map(|imported| format!("{}{}", imported.path, suffix))
        .collect::<Vec<_>>();
    let bound = !names.is_empty();
    (names, bound)
}

pub fn simple_name(value: &str) -> &str {
    value.rsplit('.').next().unwrap_or(value)
}

pub fn qualify(owner: &str, name: &str) -> String {
    if owner.is_empty() || owner == "<default>" {
        name.into()
    } else {
        format!("{owner}.{name}")
    }
}

pub fn queue_supertypes(
    pending: &mut Vec<PendingRelationship>,
    file: &ParsedFile,
    source: &str,
    lexical_owner: &str,
    supertypes: &[SupertypeDecl],
) {
    for supertype in supertypes {
        pending.push(PendingRelationship {
            attributes: supertype.delegated.then(|| {
                serde_json::json!({
                    "delegate_expression": supertype.delegate_expression,
                    "delegated": true
                })
            }),
            expression: supertype.expression.clone(),
            file: file.clone(),
            kind: "supertype".into(),
            lexical_owner: lexical_owner.into(),
            span: supertype.span.clone(),
            source: source.into(),
            target_name: supertype.target_name.clone(),
        });
    }
}

pub fn queue_annotations(
    pending: &mut Vec<PendingRelationship>,
    file: &ParsedFile,
    source: &str,
    lexical_owner: &str,
    annotations: &[String],
    span: &SourceSpan,
) {
    for annotation in annotations {
        pending.push(PendingRelationship {
            attributes: None,
            expression: annotation.clone(),
            file: file.clone(),
            kind: "annotation".into(),
            lexical_owner: lexical_owner.into(),
            span: span.clone(),
            source: source.into(),
            target_name: annotation_reference(annotation).into(),
        });
    }
}

pub fn emit_relationships(
    facts: &mut Facts,
    index: &BTreeMap<String, Vec<RelationshipTarget>>,
    pending: &[PendingRelationship],
) {
    for relation in pending {
        let (targets, reason) = resolve_relationship(index, relation);
        let mut kind = if relation.kind == "supertype" {
            "extends"
        } else {
            "annotates"
        };
        if targets.len() == 1 {
            if relation.kind == "supertype" && targets[0].kind == "interface" {
                kind = "implements";
            }
            facts.add_edge(
                &relation.source,
                &targets[0].id,
                kind,
                Some(&relation.file.path),
                Some(relation.span.clone()),
                relation.attributes.clone(),
            );
        } else {
            facts.add_unresolved(
                &relation.source,
                kind,
                &relation.expression,
                &reason,
                Some(&relation.file.path),
                Some(relation.span.clone()),
                relation.attributes.clone(),
            );
        }
    }
}

fn resolve_relationship(
    index: &BTreeMap<String, Vec<RelationshipTarget>>,
    pending: &PendingRelationship,
) -> (Vec<RelationshipTarget>, String) {
    let name = &pending.target_name;
    if name.is_empty() {
        return (Vec::new(), "unsupported-form".into());
    }
    let annotations_only = pending.kind == "annotation";
    let resolve = |names: Vec<String>| -> Vec<RelationshipTarget> {
        unique(
            names
                .into_iter()
                .flat_map(|name| index.get(&name).into_iter().flatten().cloned())
                .filter(|target| !annotations_only || target.form == "annotation_class")
                .collect(),
        )
    };
    let classify = |targets: Vec<RelationshipTarget>| {
        let reason = match targets.len() {
            0 => "external-target",
            1 => "",
            _ => "ambiguous-target",
        };
        (targets, reason.into())
    };

    if name.contains('.') {
        let targets = resolve(vec![name.clone()]);
        if !targets.is_empty() {
            return classify(targets);
        }
    }
    let (aliases, bound) = imported_alias_names(&pending.file, name);
    if bound {
        return classify(resolve(aliases));
    }

    let package = if pending.file.package_name.is_empty() {
        "<default>"
    } else {
        &pending.file.package_name
    };
    let mut owner = pending.lexical_owner.as_str();
    while !owner.is_empty() && owner != "<default>" && owner != package {
        let targets = resolve(vec![format!("{owner}.{name}")]);
        if !targets.is_empty() {
            return classify(targets);
        }
        owner = owner.rsplit_once('.').map_or("", |(parent, _)| parent);
    }

    let (imports, bound) = explicit_import_names(&pending.file, name);
    if bound {
        return classify(resolve(imports));
    }
    let local = resolve(vec![qualify(package, name)]);
    if !local.is_empty() {
        return classify(local);
    }

    let wildcard = pending
        .file
        .imports
        .iter()
        .filter(|imported| imported.wildcard)
        .map(|imported| format!("{}.{}", imported.path.trim_end_matches(".*"), name))
        .collect();
    classify(resolve(wildcard))
}

fn unique(values: Vec<RelationshipTarget>) -> Vec<RelationshipTarget> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.id.clone()))
        .collect()
}
