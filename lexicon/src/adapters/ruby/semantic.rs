#![allow(clippy::collapsible_if)]

use crate::{EdgeRecord, FactRecord, NodeRecord, UnresolvedRecord, node_id};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Clone)]
enum Frame {
    Namespace(String),
    Method {
        id: String,
        qualified: String,
        owner: String,
    },
    Block {
        id: String,
    },
    Other,
}

pub fn enrich(root: &Path, records: &mut Vec<FactRecord>) {
    let mut files = Vec::new();
    collect(root, root, &mut files);
    files.sort();

    let mut inheritance = BTreeMap::<String, String>::new();
    for record in records.iter() {
        if let FactRecord::Edge(edge) = record {
            if edge.relation == "extends" {
                if let (Some(child), Some(parent)) = (
                    qualified_for(records, &edge.source),
                    qualified_for(records, &edge.target),
                ) {
                    inheritance.insert(child, parent);
                }
            }
        }
    }

    let mut mixins = BTreeMap::<String, Vec<String>>::new();
    let mut return_types = BTreeMap::<String, String>::new();
    let mut param_types = BTreeMap::<String, BTreeMap<String, BTreeSet<String>>>::new();

    for path in &files {
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        discover_semantics(
            root,
            path,
            &source,
            records,
            &mut mixins,
            &mut return_types,
            &mut param_types,
        );
    }

    for path in files {
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        emit_semantics(
            root,
            &path,
            &source,
            records,
            &inheritance,
            &mixins,
            &return_types,
            &param_types,
        );
    }
}

include!("semantic_discovery.rs");
include!("semantic_emission.rs");
include!("semantic_resolution.rs");
include!("semantic_syntax.rs");
include!("semantic_support.rs");
