use std::path::PathBuf;

use lexicon::adapters::kotlin::KotlinAdapter;
use lexicon::{AdapterHost, AdapterRequest, FactRecord, LanguageAdapter};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("adapters")
        .join("kotlin")
        .join("testdata")
        .join(name)
}

fn analyze(name: &str) -> lexicon::Analysis {
    let mut analysis = KotlinAdapter
        .analyze(&AdapterRequest {
            language: "kotlin".into(),
            repository: fixture(name),
            ..Default::default()
        })
        .unwrap();
    analysis.canonicalize().unwrap();
    analysis.validate().unwrap();
    analysis
}

#[test]
fn kotlin_is_registered_by_default_with_native_fingerprint() {
    let host = AdapterHost::new(fixture("foundation").join("unused-adapters"));
    assert!(host.has_adapter("kotlin"));
    assert!(host.fingerprint("kotlin").unwrap().starts_with("sha256:"));
}

#[test]
fn kotlin_foundation_matches_structural_oracle_surface() {
    let analysis = analyze("foundation");
    let domain = "src/main/kotlin/demo/model/Domain.kt";

    for kind in [
        "repository",
        "directory",
        "file",
        "module",
        "namespace",
        "symbol",
        "type",
        "interface",
        "function",
        "method",
        "constructor",
        "field",
        "parameter",
        "import",
    ] {
        assert!(
            analysis.records.iter().any(|record| matches!(
                record,
                FactRecord::Node(node) if node.kind == kind
            )),
            "missing node kind {kind}"
        );
    }

    let user = node(&analysis.records, "type", "User", domain);
    assert_eq!(
        attribute(user, "declaration_kind"),
        Some(&serde_json::json!("data_class"))
    );
    assert_eq!(
        user.id,
        lexicon::node_id("kotlin", "type", &format!("source:{domain}::type:User"))
    );

    let result = node(&analysis.records, "interface", "Result", domain);
    assert_eq!(
        attribute(result, "declaration_kind"),
        Some(&serde_json::json!("sealed_interface"))
    );
    assert_eq!(
        attribute(
            node(&analysis.records, "type", "Mode", domain),
            "declaration_kind"
        ),
        Some(&serde_json::json!("enum_class"))
    );
    assert_eq!(
        attribute(
            node(&analysis.records, "type", "Registry", domain),
            "declaration_kind"
        ),
        Some(&serde_json::json!("object"))
    );
    assert_eq!(
        attribute(
            node(&analysis.records, "type", "Factory", domain),
            "declaration_kind"
        ),
        Some(&serde_json::json!("companion_object"))
    );
    assert_eq!(
        attribute(
            node(&analysis.records, "type", "UserId", domain),
            "declaration_kind"
        ),
        Some(&serde_json::json!("value_class"))
    );

    let decode = node(&analysis.records, "method", "decode", domain);
    assert_eq!(attribute(decode, "suspend"), Some(&serde_json::json!(true)));
    assert_eq!(
        attribute(decode, "extension_receiver"),
        Some(&serde_json::json!("String?"))
    );
    assert_eq!(
        attribute(decode, "return_type"),
        Some(&serde_json::json!("User?"))
    );

    assert_eq!(
        nodes(&analysis.records, "constructor", "User", domain).len(),
        2
    );
    let empty = node(&analysis.records, "constructor", "Empty", domain);
    assert!(
        attribute(empty, "modifiers")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|values| values.contains(&serde_json::json!("implicit")))
    );

    let id = node(&analysis.records, "field", "id", domain);
    assert_eq!(
        attribute(id, "constructor_parameter"),
        Some(&serde_json::json!(true))
    );
    assert_eq!(attribute(id, "mutable"), Some(&serde_json::json!(false)));

    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.relation == "defines" && value.reason == "unsupported-form"
    )));
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node) if node.name == "broken" && node.kind == "function"
    )));
}

#[test]
fn kotlin_relationship_resolution_matches_oracle() {
    let analysis = analyze("relationships");
    for (source, target, relation) in [
        (
            "relationships.app.AliasChild",
            "relationships.contracts.Base",
            "extends",
        ),
        (
            "relationships.app.AliasChild",
            "relationships.contracts.Contract",
            "implements",
        ),
        (
            "relationships.app.AliasChild",
            "relationships.contracts.Marker",
            "annotates",
        ),
        (
            "relationships.app.WildChild",
            "relationships.wild.WildContract",
            "implements",
        ),
        (
            "relationships.app.NestedAliasChild",
            "relationships.contracts.Outer.NestedContract",
            "implements",
        ),
        (
            "relationships.app.Lexical.InnerChild",
            "relationships.app.Lexical.InnerContract",
            "implements",
        ),
    ] {
        assert_relation(&analysis.records, source, target, relation);
    }

    assert_unresolved(
        &analysis.records,
        "relationships.app.AmbiguousChild",
        "extends",
        "Shared",
        "ambiguous-target",
    );
    assert_unresolved(
        &analysis.records,
        "relationships.app.ExternalChild",
        "extends",
        "external.Base()",
        "external-target",
    );
}

#[test]
fn kotlin_dependency_manifests_match_oracle() {
    let analysis = analyze("dependencies");
    let depends = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(edge) if edge.relation == "depends-on" => Some(edge),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(depends.len(), 15);

    for (coordinate, configuration) in [
        (
            "org.jetbrains.kotlin:kotlin-stdlib:2.0.21",
            "implementation",
        ),
        ("com.example:public-api:1.0", "api"),
        ("org.jetbrains:annotations:24.1.0", "compileOnly"),
        ("com.example:runtime:3.0", "runtimeOnly"),
        (
            "org.junit.jupiter:junit-jupiter:5.11.0",
            "testImplementation",
        ),
        ("com.google.dagger:dagger-compiler:2.52", "kapt"),
        (
            "com.google.devtools.ksp:symbol-processing-api:2.0.21-1.0.28",
            "ksp",
        ),
        ("org.codehaus.groovy:groovy:3.0.22", "implementation"),
        ("com.acme:groovy-api:1.2", "api"),
        ("com.acme:runtime:2.0", "runtimeOnly"),
        ("org.slf4j:slf4j-api:2.0.16", "compile"),
        ("org.junit.jupiter:junit-jupiter:5.11.0", "test"),
        ("com.example:versionless", ""),
    ] {
        assert!(
            depends.iter().any(|edge| {
                let attrs = edge.attributes.as_ref().unwrap();
                attrs.get("coordinate") == Some(&serde_json::json!(coordinate))
                    && attrs.get("configuration") == Some(&serde_json::json!(configuration))
            }),
            "missing dependency {configuration} {coordinate}"
        );
    }

    let public_api = depends
        .iter()
        .filter(|edge| {
            edge.attributes
                .as_ref()
                .and_then(|attrs| attrs.get("coordinate"))
                == Some(&serde_json::json!("com.example:public-api:1.0"))
        })
        .count();
    assert_eq!(public_api, 2);
    let slf4j = depends
        .iter()
        .filter(|edge| {
            edge.attributes
                .as_ref()
                .and_then(|attrs| attrs.get("coordinate"))
                == Some(&serde_json::json!("org.slf4j:slf4j-api:2.0.16"))
        })
        .count();
    assert_eq!(slf4j, 2);

    for expression in [
        "libs.kotlin.coroutines",
        "com.example:dynamic:",
        "project(\":shared\")",
        "platform(\"com.example:bom:1.0\")",
        "files(\"libs/local.jar\")",
        "group = \"com.example\"",
        "property-version",
        "fixture-bom",
    ] {
        assert!(
            analysis.records.iter().any(|record| matches!(
                record,
                FactRecord::Unresolved(value)
                    if value.relation == "depends-on"
                        && value.reason == "unsupported-form"
                        && value.expression.contains(expression)
            )),
            "missing unresolved dependency {expression}"
        );
    }

    for ignored in ["managed-bom", "profile-only", "plugin", "ignored-parent"] {
        assert!(
            !analysis.records.iter().any(|record| matches!(
                record,
                FactRecord::Node(node)
                    if node.kind == "module"
                        && (node.name == ignored || node.qualified_name.contains(ignored))
            )),
            "unsupported Maven section emitted {ignored}"
        );
    }
}

#[test]
fn kotlin_runtime_calls_overrides_and_dataflow_match_oracle() {
    let analysis = analyze("runtime");
    let run = qualified_node(
        &analysis.records,
        "method",
        "runtime.slice.Child.run(Int,ExternalWorker)",
    );
    for target in [
        "runtime.slice.Child.helper(Int)",
        "runtime.slice.top(Int)",
        "runtime.slice.Helpers.work(Int)",
        "runtime.slice.Factory.Companion.create(Int)",
        "runtime.slice.Child.<init>(Int)",
    ] {
        assert_runtime_edge(
            &analysis.records,
            run,
            qualified_node_any(&analysis.records, target),
            "calls",
        );
    }
    for target in ["runtime.slice.choose(Int)", "runtime.slice.choose(String)"] {
        let target = qualified_node(&analysis.records, "function", target);
        assert_runtime_edge(&analysis.records, run, target, "possible-calls");
        assert_no_runtime_edge(&analysis.records, run, target, "calls");
    }

    let secondary_int = qualified_node(
        &analysis.records,
        "constructor",
        "runtime.slice.Secondary.<init>(Int)",
    );
    let secondary_empty = qualified_node(
        &analysis.records,
        "constructor",
        "runtime.slice.Secondary.<init>()",
    );
    assert_runtime_edge(
        &analysis.records,
        secondary_int,
        qualified_node(
            &analysis.records,
            "constructor",
            "runtime.slice.Parent.<init>(Int)",
        ),
        "calls",
    );
    assert_runtime_edge(&analysis.records, secondary_empty, secondary_int, "calls");
    assert_runtime_unresolved(&analysis.records, run, "worker.run()", "dynamic-target");
    assert_runtime_unresolved(&analysis.records, run, "println(input)", "external-target");
    assert_runtime_unresolved(&analysis.records, run, "unsupported()", "unsupported-form");

    let compute = qualified_node(
        &analysis.records,
        "method",
        "runtime.slice.Child.compute(Int)",
    );
    assert_runtime_edge(
        &analysis.records,
        compute,
        qualified_node(
            &analysis.records,
            "method",
            "runtime.slice.Base.compute(Int)",
        ),
        "overrides",
    );
    assert_runtime_edge(
        &analysis.records,
        compute,
        qualified_node(
            &analysis.records,
            "method",
            "runtime.slice.Contract.compute(Int)",
        ),
        "overrides",
    );
    assert_runtime_edge(
        &analysis.records,
        qualified_node(
            &analysis.records,
            "method",
            "runtime.slice.Child.render(Int)",
        ),
        qualified_node(
            &analysis.records,
            "method",
            "runtime.slice.Base.render(Int)",
        ),
        "overrides",
    );
    assert_runtime_edge(
        &analysis.records,
        secondary_int,
        qualified_node_any(
            &analysis.records,
            "runtime.slice.Secondary.<init>(Int)::parameter:value",
        ),
        "reads",
    );

    for target in [
        "runtime.slice.Child.count",
        "runtime.slice.Helpers.state",
        "runtime.slice.Child.run(Int,ExternalWorker)::parameter:input",
        "runtime.slice.Child.run(Int,ExternalWorker)::parameter:worker",
    ] {
        assert_runtime_edge(
            &analysis.records,
            run,
            qualified_node_any(&analysis.records, target),
            "reads",
        );
    }
    for target in ["runtime.slice.Child.count", "runtime.slice.Helpers.state"] {
        assert_runtime_edge(
            &analysis.records,
            run,
            qualified_node_any(&analysis.records, target),
            "writes",
        );
    }

    let delegated = qualified_node(&analysis.records, "field", "runtime.slice.Child.delegated");
    for source in [
        "runtime.slice.Child.shadows(Int)",
        "runtime.slice.Child.destructured(Pair<Int, Int>)",
        "runtime.slice.Child.delegatedLocal()",
        "runtime.slice.Child.nestedLocal()",
        "runtime.slice.Child.nestedLambda()",
    ] {
        let source = qualified_node(&analysis.records, "method", source);
        assert_no_runtime_edge(&analysis.records, source, delegated, "reads");
        assert_no_runtime_edge(&analysis.records, source, delegated, "writes");
        let count = qualified_node(&analysis.records, "field", "runtime.slice.Child.count");
        assert_no_runtime_edge(&analysis.records, source, count, "reads");
        assert_no_runtime_edge(&analysis.records, source, count, "writes");
    }
}

#[test]
fn kotlin_typed_receiver_extensions_match_oracle() {
    let analysis = analyze("extensions");
    let direct = qualified_node(&analysis.records, "function", "extensions.api.direct(Int)");
    for source in [
        "extensions.app.Usage.throughParameter(ModelItem)",
        "extensions.app.Usage.throughLocal()",
        "extensions.app.Usage.throughProperty()",
        "extensions.app.Usage.shadowing(ModelItem,Boolean)",
    ] {
        assert_runtime_edge(
            &analysis.records,
            qualified_node(&analysis.records, "method", source),
            direct,
            "calls",
        );
    }

    let through_parameter = qualified_node(
        &analysis.records,
        "method",
        "extensions.app.Usage.throughParameter(ModelItem)",
    );
    for target in [
        "extensions.api.defaulted(Int)",
        "extensions.api.spread(Int)",
    ] {
        assert_runtime_edge(
            &analysis.records,
            through_parameter,
            qualified_node(&analysis.records, "function", target),
            "calls",
        );
    }

    let imports = qualified_node(
        &analysis.records,
        "method",
        "extensions.app.Usage.throughImports(ModelItem)",
    );
    for target in [
        "extensions.api.imported()",
        "extensions.wild.wild()",
        "extensions.app.Usage.lexical(Int)",
        "extensions.app.samePackage()",
    ] {
        assert_runtime_edge(
            &analysis.records,
            imports,
            qualified_node_any(&analysis.records, target),
            "calls",
        );
    }

    let ambiguous = qualified_node(
        &analysis.records,
        "method",
        "extensions.app.Usage.ambiguous(ModelItem)",
    );
    for target in [
        "extensions.api.ambiguous(Int)",
        "extensions.api.ambiguous(String)",
    ] {
        let target = qualified_node(&analysis.records, "function", target);
        assert_runtime_edge(&analysis.records, ambiguous, target, "possible-calls");
        assert_no_runtime_edge(&analysis.records, ambiguous, target, "calls");
    }

    let external = qualified_node(
        &analysis.records,
        "method",
        "extensions.app.Usage.external(String,ModelItem)",
    );
    assert_runtime_unresolved(
        &analysis.records,
        external,
        "text.externalOnly()",
        "dynamic-target",
    );
    assert_runtime_unresolved(
        &analysis.records,
        external,
        "item.thirdParty()",
        "external-target",
    );

    let shadowing = qualified_node(
        &analysis.records,
        "method",
        "extensions.app.Usage.shadowing(ModelItem,Boolean)",
    );
    let direct_count = analysis
        .records
        .iter()
        .filter(|record| {
            matches!(
                record,
                FactRecord::Edge(edge)
                    if edge.source == shadowing.id
                        && edge.target == direct.id
                        && edge.relation == "calls"
            )
        })
        .count();
    assert_eq!(direct_count, 3);
    assert_runtime_unresolved(
        &analysis.records,
        shadowing,
        "item.direct(6)",
        "dynamic-target",
    );
    assert_runtime_unresolved(
        &analysis.records,
        shadowing,
        "property.direct(7)",
        "dynamic-target",
    );

    let member = qualified_node(
        &analysis.records,
        "method",
        "extensions.app.Usage.memberWins(ModelItem)",
    );
    assert_runtime_unresolved(
        &analysis.records,
        member,
        "item.collision()",
        "dynamic-target",
    );

    let unsupported = qualified_node(
        &analysis.records,
        "method",
        "extensions.app.Usage.unsupported(ModelItem,Any)",
    );
    for (expression, reason) in [
        ("item.direct(1)", "dynamic-target"),
        ("fluent()", "unsupported-form"),
        ("direct(2)", "unsupported-form"),
        ("dynamic.direct(3)", "dynamic-target"),
        ("inferred.direct(4)", "dynamic-target"),
    ] {
        assert_runtime_unresolved(&analysis.records, unsupported, expression, reason);
    }
}

#[test]
fn kotlin_structural_analysis_is_deterministic() {
    for fixture in [
        "foundation",
        "relationships",
        "dependencies",
        "runtime",
        "extensions",
    ] {
        assert_eq!(
            analyze(fixture),
            analyze(fixture),
            "fixture {fixture} changed output"
        );
    }
}

fn node<'a>(
    records: &'a [FactRecord],
    kind: &str,
    name: &str,
    path: &str,
) -> &'a lexicon::NodeRecord {
    nodes(records, kind, name, path)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("missing {kind} {path}::{name}"))
}

fn nodes<'a>(
    records: &'a [FactRecord],
    kind: &str,
    name: &str,
    path: &str,
) -> Vec<&'a lexicon::NodeRecord> {
    records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node)
                if node.kind == kind && node.name == name && node.path == path =>
            {
                Some(node)
            }
            _ => None,
        })
        .collect()
}

fn attribute<'a>(node: &'a lexicon::NodeRecord, name: &str) -> Option<&'a serde_json::Value> {
    node.attributes.as_ref()?.get(name)
}

fn typed_node<'a>(records: &'a [FactRecord], qualified: &str) -> &'a lexicon::NodeRecord {
    records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node)
                if node.qualified_name == qualified
                    && matches!(node.kind.as_str(), "type" | "interface") =>
            {
                Some(node)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing typed node {qualified}"))
}

fn assert_relation(records: &[FactRecord], source: &str, target: &str, relation: &str) {
    let source = typed_node(records, source);
    let target = typed_node(records, target);
    assert!(records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.source == source.id
                && edge.target == target.id
                && edge.relation == relation
    )));
}

fn assert_unresolved(
    records: &[FactRecord],
    source: &str,
    relation: &str,
    expression: &str,
    reason: &str,
) {
    let source = typed_node(records, source);
    assert!(records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.source == source.id
                && value.relation == relation
                && value.expression == expression
                && value.reason == reason
    )));
}
fn qualified_node<'a>(
    records: &'a [FactRecord],
    kind: &str,
    qualified: &str,
) -> &'a lexicon::NodeRecord {
    records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.kind == kind && node.qualified_name == qualified => {
                Some(node)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {kind} {qualified}"))
}

fn qualified_node_any<'a>(records: &'a [FactRecord], qualified: &str) -> &'a lexicon::NodeRecord {
    records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.qualified_name == qualified => Some(node),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing node {qualified}"))
}

fn assert_runtime_edge(
    records: &[FactRecord],
    source: &lexicon::NodeRecord,
    target: &lexicon::NodeRecord,
    relation: &str,
) {
    assert!(
        records.iter().any(|record| matches!(
            record,
            FactRecord::Edge(edge)
                if edge.source == source.id
                    && edge.target == target.id
                    && edge.relation == relation
        )),
        "missing {relation}: {} -> {}",
        source.qualified_name,
        target.qualified_name
    );
}

fn assert_no_runtime_edge(
    records: &[FactRecord],
    source: &lexicon::NodeRecord,
    target: &lexicon::NodeRecord,
    relation: &str,
) {
    assert!(
        !records.iter().any(|record| matches!(
            record,
            FactRecord::Edge(edge)
                if edge.source == source.id
                    && edge.target == target.id
                    && edge.relation == relation
        )),
        "unexpected {relation}: {} -> {}",
        source.qualified_name,
        target.qualified_name
    );
}

fn assert_runtime_unresolved(
    records: &[FactRecord],
    source: &lexicon::NodeRecord,
    expression: &str,
    reason: &str,
) {
    assert!(
        records.iter().any(|record| matches!(
            record,
            FactRecord::Unresolved(value)
                if value.source == source.id
                    && value.relation == "calls"
                    && value.expression == expression
                    && value.reason == reason
        )),
        "missing unresolved {reason} {expression} from {}",
        source.qualified_name
    );
}
