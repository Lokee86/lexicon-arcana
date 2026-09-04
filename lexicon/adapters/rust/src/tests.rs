use crate::{emit, orchestrator};
use serde_json::Value;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample")
}

fn records() -> Vec<Value> {
    orchestrator::generate(&fixture(), None, None)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn node_id<'a>(records: &'a [Value], qualified_name: &str) -> &'a str {
    records
        .iter()
        .find(|record| record["record"] == "node" && record["qualified_name"] == qualified_name)
        .and_then(|record| record["id"].as_str())
        .unwrap_or_else(|| panic!("missing node {qualified_name}"))
}

fn has_edge(records: &[Value], source: &str, target: &str, relation: &str) -> bool {
    records.iter().any(|record| {
        record["record"] == "edge"
            && record["relation"] == relation
            && record["source"] == source
            && record["target"] == target
    })
}

fn edge_count(records: &[Value], source: &str, target: &str, relation: &str) -> usize {
    records
        .iter()
        .filter(|record| {
            record["record"] == "edge"
                && record["relation"] == relation
                && record["source"] == source
                && record["target"] == target
        })
        .count()
}

#[test]
fn emits_conservative_dataflow_for_rust_locals_parameters_fields_constants_and_shadowing() {
    let records = records();
    let flow = node_id(&records, "lexicon_fixture::lexicon_fixture::flow");
    let inner = node_id(&records, "lexicon_fixture::lexicon_fixture::inner");
    let update = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::FlowBox::update",
    );
    let field = node_id(&records, "lexicon_fixture::lexicon_fixture::FlowBox::field");
    let constant = node_id(&records, "lexicon_fixture::lexicon_fixture::FLOW_CONST");
    let targets = |source: &str, relation: &str| {
        records
            .iter()
            .filter(|record| {
                record["record"] == "edge"
                    && record["source"] == source
                    && record["relation"] == relation
            })
            .map(|record| record["target"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let flow_reads = targets(flow, "reads");
    let flow_writes = targets(flow, "writes");
    assert!(flow_reads.contains(constant));
    assert!(flow_writes.contains(field));
    assert!(flow_reads.iter().any(|target| records
        .iter()
        .any(|node| node["record"] == "node" && node["id"] == *target && node["name"] == "local")));
    assert!(flow_writes.iter().any(|target| records
        .iter()
        .any(|node| node["record"] == "node" && node["id"] == *target && node["name"] == "local")));
    assert!(targets(update, "reads").contains(field));
    assert!(targets(update, "writes").contains(field));
    assert!(targets(inner, "reads").iter().any(|target| records
        .iter()
        .any(|node| node["record"] == "node" && node["id"] == *target && node["name"] == "value")));
    let node_ids: std::collections::BTreeSet<_> = records
        .iter()
        .filter(|record| record["record"] == "node")
        .map(|record| record["id"].as_str().unwrap())
        .collect();
    assert!(records
        .iter()
        .filter(|record| record["record"] == "edge"
            && ["reads", "writes"].contains(&record["relation"].as_str().unwrap()))
        .all(|record| node_ids.contains(record["target"].as_str().unwrap())));
}

#[test]
fn indexes_rust_declarations_imports_traits_and_local_macros() {
    let records = records();
    assert!(records.iter().any(|record| {
        record["record"] == "edge"
            && record["relation"] == "depends-on"
            && record["attributes"]["path"] == true
            && record["attributes"]["category"] == "runtime"
    }));
    for (kind, name) in [
        ("type", "Service"),
        ("type", "Worker"),
        ("trait", "Runnable"),
        ("function", "generated"),
        ("function", "Data"),
        ("import", "use crate :: child"),
    ] {
        assert!(
            records.iter().any(|record| {
                record["record"] == "node"
                    && record["kind"] == kind
                    && record["name"]
                        .as_str()
                        .is_some_and(|value| value.contains(name))
            }),
            "missing {kind} {name}"
        );
    }
    assert!(records
        .iter()
        .any(|record| record["record"] == "edge" && record["relation"] == "implements"));
    assert!(records
        .iter()
        .any(|record| record["record"] == "edge" && record["relation"] == "imports"));
    let root_module = node_id(&records, "lexicon_fixture::lexicon_fixture");
    let child_module = node_id(&records, "lexicon_fixture::lexicon_fixture::child");
    assert!(records.iter().any(|record| {
        record["record"] == "edge"
            && record["relation"] == "depends-on"
            && record["source"] == root_module
            && record["target"] == child_module
            && record["attributes"]["category"] == "local"
            && record["attributes"]["path"] == true
    }));
    for (category, source_fragment) in [
        ("runtime", "external_fixture"),
        ("development", "dev_fixture"),
        ("build", "build_fixture"),
        ("runtime", "target_fixture"),
    ] {
        assert!(
            records.iter().any(|record| {
                record["record"] == "edge"
                    && record["relation"] == "depends-on"
                    && record["attributes"]["category"] == category
                    && record["attributes"]["source"]
                        .as_str()
                        .is_some_and(|source| source.contains(source_fragment))
            }),
            "missing {category} dependency {source_fragment}"
        );
    }
    assert!(records.iter().any(|record| {
        record["record"] == "edge"
            && record["relation"] == "depends-on"
            && record["attributes"]["target"] == "cfg(unix)"
    }));
    for node in records.iter().filter(|record| {
        record["record"] == "node"
            && record["qualified_name"]
                .as_str()
                .is_some_and(|name| name.starts_with("dependency:rust:"))
    }) {
        let path = node["path"].as_str().unwrap();
        assert!(path.starts_with("@dependencies/rust/"));
        assert!(!path.contains('\\'));
        assert!(!std::path::Path::new(path).is_absolute());
    }
}

#[test]
fn resolves_inherent_field_alias_constructor_ufcs_and_macro_calls() {
    let records = records();
    let top = node_id(&records, "lexicon_fixture::lexicon_fixture::top");
    for target in [
        "lexicon_fixture::lexicon_fixture::Service::factory",
        "lexicon_fixture::lexicon_fixture::Service::run_local",
        "lexicon_fixture::lexicon_fixture::Service::Runnable::run",
        "lexicon_fixture::lexicon_fixture::child::Worker::new",
        "lexicon_fixture::lexicon_fixture::child::Worker::work",
        "lexicon_fixture::lexicon_fixture::child::helper",
        "lexicon_fixture::lexicon_fixture::generated",
    ] {
        let target = node_id(&records, target);
        assert!(
            has_edge(&records, top, target, "calls")
                || has_edge(&records, top, target, "possible-calls"),
            "missing top call to {target}"
        );
    }
    let run_local = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::Service::run_local",
    );
    let work = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::child::Worker::work",
    );
    assert!(has_edge(&records, run_local, work, "calls"));
    let trait_run = node_id(&records, "lexicon_fixture::lexicon_fixture::Runnable::run");
    let service_run = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::Service::Runnable::run",
    );
    let alternate_run = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::Alternate::Runnable::run",
    );
    assert_eq!(edge_count(&records, service_run, trait_run, "overrides"), 1);
    assert_eq!(
        edge_count(&records, alternate_run, trait_run, "overrides"),
        1
    );
    let factory = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::Service::factory",
    );
    let new = node_id(&records, "lexicon_fixture::lexicon_fixture::Service::new");
    assert!(has_edge(&records, factory, new, "calls"));
    let enum_build = node_id(&records, "lexicon_fixture::lexicon_fixture::enum_build");
    let variant = node_id(&records, "lexicon_fixture::lexicon_fixture::Kind::Data");
    assert!(has_edge(&records, enum_build, variant, "calls"));
}

#[test]
fn propagates_callbacks_and_generic_trait_dispatch_as_possible_calls() {
    let records = records();
    let invoke = node_id(&records, "lexicon_fixture::lexicon_fixture::invoke");
    let helper = node_id(&records, "lexicon_fixture::lexicon_fixture::helper");
    assert!(has_edge(&records, invoke, helper, "possible-calls"));
    assert!(records.iter().any(|record| {
        record["record"] == "edge"
            && record["source"] == invoke
            && record["relation"] == "possible-calls"
            && records.iter().any(|node| {
                node["record"] == "node"
                    && node["id"] == record["target"]
                    && node["name"]
                        .as_str()
                        .is_some_and(|name| name.starts_with("closure@"))
            })
    }));
    let runner = node_id(&records, "lexicon_fixture::lexicon_fixture::invoke_runner");
    let service_run = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::Service::Runnable::run",
    );
    assert!(has_edge(&records, runner, service_run, "possible-calls"));
}

#[test]
fn resolves_local_methods_through_standard_combinators_and_trait_impls() {
    let records = records();
    let info = node_id(&records, "lexicon_fixture::lexicon_fixture::Snapshot::info");
    let closure = records
        .iter()
        .find(|record| {
            record["record"] == "node"
                && record["kind"] == "function"
                && record["qualified_name"].as_str().is_some_and(|name| {
                    name.starts_with("lexicon_fixture::lexicon_fixture::map_snapshot::closure@")
                })
        })
        .and_then(|record| record["id"].as_str())
        .expect("missing map_snapshot closure node");
    assert!(
        has_edge(&records, closure, info, "calls")
            || has_edge(&records, closure, info, "possible-calls"),
        "missing Result::map closure call to Snapshot::info"
    );

    let partial_cmp = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::Ordered::PartialOrd::partial_cmp",
    );
    let cmp = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::Ordered::Ord::cmp",
    );
    assert!(
        has_edge(&records, partial_cmp, cmp, "calls")
            || has_edge(&records, partial_cmp, cmp, "possible-calls"),
        "missing concrete Self dispatch to external-trait implementation"
    );
}

#[test]
fn classifies_standard_collection_calls_as_builtin_not_dynamic() {
    let records = records();
    let source = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::builtin_collections",
    );
    let calls: Vec<_> = records
        .iter()
        .filter(|record| {
            record["record"] == "unresolved"
                && record["source"] == source
                && record["relation"] == "calls"
        })
        .collect();
    assert!(!calls.is_empty());
    assert!(calls
        .iter()
        .all(|record| record["reason"] == "builtin-target"));
}

#[test]
fn preserves_parseable_pointer_reference_and_generic_type_text() {
    let records = records();
    let source = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::builtin_type_text",
    );
    let builtin_map = node_id(&records, "lexicon_fixture::lexicon_fixture::builtin_map");
    assert!(has_edge(&records, source, builtin_map, "calls"));
    let unresolved: Vec<_> = records
        .iter()
        .filter(|record| {
            record["record"] == "unresolved"
                && record["source"] == source
                && record["relation"] == "calls"
        })
        .collect();
    assert!(!unresolved.is_empty());
    assert!(unresolved
        .iter()
        .all(|record| record["reason"] == "builtin-target"));
}

#[test]
fn resolves_typed_values_and_standard_callback_inputs() {
    let records = records();
    let source = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::builtin_value_flow",
    );
    let dynamic_calls: Vec<_> = records
        .iter()
        .filter(|record| {
            record["record"] == "unresolved"
                && record["relation"] == "calls"
                && record["reason"] == "dynamic-target"
                && (record["source"] == source
                    || record["qualified_name"]
                        .as_str()
                        .is_some_and(|name| name.contains("builtin_value_flow::closure@")))
        })
        .collect();
    assert!(
        dynamic_calls.is_empty(),
        "unexpected dynamic calls: {dynamic_calls:?}"
    );

    let code = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::LocalError::code",
    );
    let map_error_closure = records
        .iter()
        .find(|record| {
            record["record"] == "node"
                && record["qualified_name"].as_str().is_some_and(|name| {
                    name.starts_with("lexicon_fixture::lexicon_fixture::map_error_value::closure@")
                })
        })
        .and_then(|record| record["id"].as_str())
        .expect("missing map_error_value closure node");
    assert!(
        has_edge(&records, map_error_closure, code, "calls")
            || has_edge(&records, map_error_closure, code, "possible-calls"),
        "missing map_err error-value call to LocalError::code"
    );
}

#[test]
fn classifies_generated_macros_and_standard_runtime_flow_without_false_dynamics() {
    let records = records();
    let source = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::final_runtime_calibration",
    );
    let closure_prefix = "lexicon_fixture::lexicon_fixture::final_runtime_calibration::closure@";
    let source_ids: std::collections::BTreeSet<_> = records
        .iter()
        .filter(|record| {
            record["record"] == "node"
                && (record["id"] == source
                    || record["qualified_name"]
                        .as_str()
                        .is_some_and(|name| name.starts_with(closure_prefix)))
        })
        .filter_map(|record| record["id"].as_str())
        .collect();
    let dynamic_calls: Vec<_> = records
        .iter()
        .filter(|record| {
            record["record"] == "unresolved"
                && record["relation"] == "calls"
                && record["reason"] == "dynamic-target"
                && record["source"]
                    .as_str()
                    .is_some_and(|id| source_ids.contains(id))
        })
        .collect();
    assert!(
        dynamic_calls.is_empty(),
        "unexpected dynamic calls: {dynamic_calls:?}"
    );
    assert!(records.iter().any(|record| {
        record["record"] == "unresolved"
            && record["source"] == source
            && record["relation"] == "calls"
            && record["reason"] == "generated-target"
            && record["expression"]
                .as_str()
                .is_some_and(|expression| expression.starts_with("field !"))
    }));

    let finish = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::LocalHasher::finish",
    );
    assert!(source_ids.iter().any(|source_id| {
        has_edge(&records, source_id, finish, "calls")
            || has_edge(&records, source_id, finish, "possible-calls")
    }));
}

#[test]
fn resolves_self_variants_generated_defaults_and_closure_captures() {
    let records = records();
    let failure = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::LocalError::failure",
    );
    let variant = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::LocalError::Failure",
    );
    assert!(has_edge(&records, failure, variant, "calls"));

    let generated_default = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::generated_default",
    );
    assert!(records.iter().any(|record| {
        record["record"] == "unresolved"
            && record["source"] == generated_default
            && record["relation"] == "calls"
            && record["reason"] == "generated-target"
            && record["expression"]
                .as_str()
                .is_some_and(|expression| expression.contains("GeneratedDefault :: default"))
    }));

    let closure = records
        .iter()
        .find(|record| {
            record["record"] == "node"
                && record["qualified_name"].as_str().is_some_and(|name| {
                    name.starts_with(
                        "lexicon_fixture::lexicon_fixture::captured_snapshot::closure@",
                    )
                })
        })
        .and_then(|record| record["id"].as_str())
        .expect("missing captured_snapshot closure node");
    let info = node_id(&records, "lexicon_fixture::lexicon_fixture::Snapshot::info");
    assert!(
        has_edge(&records, closure, info, "calls")
            || has_edge(&records, closure, info, "possible-calls"),
        "missing captured Snapshot::info call"
    );
}

#[test]
fn classifies_nested_bare_function_alias_calls_as_dynamic() {
    let records = records();
    let invoke_aliases = node_id(&records, "lexicon_fixture::lexicon_fixture::invoke_aliases");
    assert!(records.iter().any(|record| {
        record["record"] == "unresolved"
            && record["source"] == invoke_aliases
            && record["relation"] == "calls"
            && record["reason"] == "dynamic-target"
            && record["expression"] == "callback (& mut bytes)"
    }));
    assert!(!records.iter().any(|record| {
        record["record"] == "unresolved"
            && record["source"] == invoke_aliases
            && record["reason"] == "missing-target"
    }));
}

#[test]
fn propagates_callable_aliases_through_tuple_container_destructuring() {
    let records = records();
    let source = node_id(
        &records,
        "lexicon_fixture::lexicon_fixture::invoke_tuple_aliases",
    );
    let dynamic_calls: Vec<_> = records
        .iter()
        .filter(|record| {
            record["record"] == "unresolved"
                && record["source"] == source
                && record["relation"] == "calls"
                && record["reason"] == "dynamic-target"
        })
        .collect();
    assert_eq!(dynamic_calls.len(), 2);
    assert!(!records.iter().any(|record| {
        record["record"] == "unresolved"
            && record["source"] == source
            && record["reason"] == "missing-target"
    }));
}

#[test]
fn classifies_only_current_unresolved_reasons() {
    let records = records();
    let reasons: Vec<_> = records
        .iter()
        .filter(|record| record["record"] == "unresolved" && record["relation"] == "calls")
        .filter_map(|record| record["reason"].as_str())
        .collect();
    assert!(reasons.contains(&"missing-target"));
    assert!(reasons.contains(&"external-target"));
    assert!(reasons.contains(&"builtin-target"));
    assert!(reasons.contains(&"generated-target"));
    for legacy in ["method-call", "associated-target", "macro-call"] {
        assert!(!reasons.contains(&legacy));
    }
    let ambiguous = node_id(&records, "lexicon_fixture::lexicon_fixture::ambiguous");
    assert_eq!(
        records
            .iter()
            .filter(|record| record["record"] == "edge"
                && record["source"] == ambiguous
                && record["relation"] == "possible-calls")
            .count(),
        2
    );
}

#[test]
fn repeat_runs_are_byte_identical_and_paths_are_relative() {
    let first = orchestrator::generate(&fixture(), None, None).unwrap();
    assert_eq!(
        first,
        orchestrator::generate(&fixture(), None, None).unwrap()
    );
    for record in first.lines().skip(1) {
        let value: Value = serde_json::from_str(record).unwrap();
        if let Some(path) = value.get("path").and_then(Value::as_str) {
            assert!(!path.contains(":\\"));
            assert!(!path.starts_with('/'));
        }
    }
}

#[test]
fn header_and_fact_order_are_canonical() {
    let output = orchestrator::generate(&fixture(), None, None).unwrap();
    assert_eq!(
        output.lines().next().unwrap(),
        r#"{"adapter_version":"0.5.0","language":"rust","record":"lexicon","repository":"lexicon_fixture","schema_version":1}"#
    );
    let records: Vec<Value> = output
        .lines()
        .skip(1)
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let mut previous = None;
    for record in records {
        let key = emit::fact_sort_key(&record);
        if let Some(previous) = &previous {
            assert!(previous <= &key);
        }
        previous = Some(key);
    }
}

#[test]
fn discovers_nested_cargo_workspace_without_root_manifest() {
    let repository =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nested_repository");
    assert!(!repository.join("Cargo.toml").exists());

    let records: Vec<Value> = orchestrator::generate(&repository, None, None)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    for package in ["nested-core", "nested-cli"] {
        assert!(
            records.iter().any(|record| {
                record["record"] == "node"
                    && record["kind"] == "module"
                    && record["attributes"]["package"] == package
            }),
            "missing nested Cargo package {package}"
        );
    }
    assert!(records.iter().any(|record| {
        record["record"] == "node"
            && record["kind"] == "file"
            && record["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("native/vector-engine/"))
    }));
}
