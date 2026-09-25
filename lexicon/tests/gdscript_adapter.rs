mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use lexicon::adapters::gdscript::GdscriptAdapter;
use lexicon::{AdapterHost, AdapterRequest, FactRecord, LanguageAdapter};

use support::TestDirectory;

#[test]
fn gdscript_is_registered_by_default_with_native_fingerprint() {
    let fixture = TestDirectory::new("gdscript-host");
    let host = AdapterHost::new(fixture.path.join("adapters"));
    assert!(host.has_adapter("gdscript"));
    let fingerprint = host.fingerprint("gdscript").unwrap();
    assert!(fingerprint.starts_with("sha256:"));
}

#[test]
fn gdscript_extracts_repository_slice_and_project_dependencies() {
    let fixture = TestDirectory::new("gdscript-slice");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "project.godot",
        "[application]\nconfig/name=\"DependencyFixture\"\n[editor_plugins]\nenabled=PackedStringArray(\"res://addons/example/plugin.cfg\")\n[autoload]\nState=\"*res://scripts/base.gd\"\n[rendering]\nresource=\"res://scenes/main.tscn\"\n",
    );
    write(
        &repo,
        "addons/example/plugin.cfg",
        "[plugin]\nname=\"Example\"\n",
    );
    write(&repo, "scenes/main.tscn", "[gd_scene]\n");
    write(
        &repo,
        "scripts/base.gd",
        "class_name Base\nextends Node\nsignal changed(value)\nconst LIMIT = 3\nvar title: String = \"# is not a comment\"\nfunc greet(name):\n    return name\n",
    );
    write(
        &repo,
        "scripts/player.gd",
        "# func ignored()\nclass_name Player extends Base\n@onready var scene = preload(\"res://scripts/base.gd\")\nsignal spawned\nfunc greet(text):\n    return text\nfunc run(\n    value: int,\n):\n    var message = \"load(\\\"res://fake.gd\\\") # string\"\n    greet(message)\n    load(get_path())\n",
    );
    for path in [
        ".worktrees/ignored.gd",
        "vendor/ignored.gd",
        ".ddocs/ignored.gd",
        ".lexicon/ignored.gd",
        ".arcana/ignored.gd",
        ".grimoire/ignored.gd",
        ".warlock/ignored.gd",
    ] {
        write(&repo, path, "class_name IgnoredState\n");
    }

    let analysis = analyze(&repo);
    let nodes = nodes(&analysis.records);
    let edges = edges(&analysis.records);
    let unresolved = unresolved(&analysis.records);

    assert_eq!(
        nodes
            .iter()
            .find(|node| node.kind == "type" && node.name == "Player")
            .unwrap()
            .id,
        lexicon::node_id("gdscript", "type", "scripts/player.gd::type::Player")
    );
    for kind in [
        "repository",
        "directory",
        "file",
        "module",
        "type",
        "function",
        "signal",
        "constant",
        "variable",
        "import",
    ] {
        assert!(nodes.iter().any(|node| node.kind == kind), "missing {kind}");
    }
    for name in [
        "Player", "Base", "greet", "run", "changed", "LIMIT", "title", "spawned",
    ] {
        assert!(nodes.iter().any(|node| node.name == name), "missing {name}");
    }
    for relation in [
        "contains",
        "defines",
        "imports",
        "references",
        "extends",
        "calls",
        "depends-on",
    ] {
        assert!(
            edges.iter().any(|edge| edge.relation == relation),
            "missing relation {relation}"
        );
    }
    assert!(
        unresolved
            .iter()
            .any(|value| value.reason == "dynamic-target")
    );
    assert!(edges.iter().any(|edge| {
        edge.relation == "depends-on"
            && edge
                .attributes
                .as_ref()
                .is_some_and(|value| value["category"] == "plugin")
    }));
    assert!(edges.iter().any(|edge| {
        edge.relation == "depends-on"
            && edge
                .attributes
                .as_ref()
                .is_some_and(|value| value["category"] == "autoload")
    }));
    assert!(!nodes.iter().any(|node| node.name == "IgnoredState"));
}

#[test]
fn gdscript_parser_handles_strings_comments_and_multiline_declarations() {
    let fixture = TestDirectory::new("gdscript-parser");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "scene.gd",
        "class_name Scene\nvar text = \"func fake() # not a comment\"\n# signal fake()\nfunc run(\n    value: int,\n    label = \"signal fake()\",\n):\n    var nested = preload(\n        \"res://other.gd\"\n    )\n",
    );
    let analysis = analyze(&repo);
    let semantic = nodes(&analysis.records)
        .into_iter()
        .filter(|node| {
            !matches!(
                node.kind.as_str(),
                "repository" | "directory" | "file" | "module" | "import" | "parameter"
            )
        })
        .map(|node| format!("{}:{}", node.kind, node.name))
        .collect::<Vec<_>>();
    for expected in [
        "type:Scene",
        "variable:text",
        "function:run",
        "variable:nested",
    ] {
        assert!(
            semantic.iter().any(|value| value == expected),
            "missing {expected}: {semantic:?}"
        );
    }
    assert!(!semantic.iter().any(|value| value.contains("fake")));
}

#[test]
fn gdscript_dataflow_is_conservative_for_updates_members_and_parameters() {
    let fixture = TestDirectory::new("gdscript-dataflow");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "box.gd",
        "class_name Box\nvar field = 0\nfunc run(value):\n    var local = value\n    local += value\n    local++\n    self.field = local\n    return local + value\nfunc inner(value):\n    return value\n",
    );
    let analysis = analyze(&repo);
    let run = node_id(&analysis.records, "function", "run", "box.gd");
    let inner = node_id(&analysis.records, "function", "inner", "box.gd");
    let symbol_ids = nodes(&analysis.records)
        .into_iter()
        .filter(|node| matches!(node.name.as_str(), "field" | "local" | "value"))
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    let mut run_reads = 0;
    let mut run_writes = 0;
    let mut inner_reads = 0;
    for edge in edges(&analysis.records) {
        if matches!(edge.relation.as_str(), "reads" | "writes") {
            assert!(
                symbol_ids.contains(&edge.target),
                "bad dataflow target {edge:?}"
            );
        }
        if edge.source == run && edge.relation == "reads" {
            run_reads += 1;
        }
        if edge.source == run && edge.relation == "writes" {
            run_writes += 1;
        }
        if edge.source == inner && edge.relation == "reads" {
            inner_reads += 1;
        }
    }
    assert!(run_reads >= 2, "run reads={run_reads}");
    assert!(run_writes >= 2, "run writes={run_writes}");
    assert!(inner_reads > 0);
}

#[test]
fn gdscript_resolves_unique_class_calls_and_keeps_dynamic_calls_unresolved() {
    let fixture = TestDirectory::new("gdscript-class-calls");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "actor.gd",
        "class_name Actor\nstatic func spawn():\n    pass\n",
    );
    write(
        &repo,
        "caller.gd",
        "class_name Caller\nfunc run(instance):\n    Actor.spawn()\n    Actor.new()\n    instance.spawn()\n",
    );
    write(
        &repo,
        "ambiguous_one.gd",
        "class_name Ambiguous\nfunc ping():\n    pass\n",
    );
    write(
        &repo,
        "ambiguous_two.gd",
        "class_name Ambiguous\nfunc ping():\n    pass\n",
    );
    write(
        &repo,
        "ambiguous_caller.gd",
        "func run():\n    Ambiguous.ping()\n    Ambiguous.new()\n",
    );

    let analysis = analyze(&repo);
    let actor = node_id(&analysis.records, "type", "Actor", "actor.gd");
    let spawn = node_id(&analysis.records, "function", "spawn", "actor.gd");
    assert!(
        edges(&analysis.records)
            .iter()
            .any(|edge| edge.relation == "calls" && edge.target == spawn)
    );
    assert!(
        edges(&analysis.records)
            .iter()
            .any(|edge| edge.relation == "calls" && edge.target == actor)
    );

    let unresolved = unresolved(&analysis.records)
        .into_iter()
        .filter(|value| value.relation == "calls")
        .map(|value| value.expression.as_str())
        .collect::<BTreeSet<_>>();
    for expected in ["instance.spawn()", "Ambiguous.ping()", "Ambiguous.new()"] {
        assert!(
            unresolved.contains(expected),
            "missing unresolved {expected}: {unresolved:?}"
        );
    }
    assert!(!unresolved.contains("Actor.spawn()"));
    assert!(!unresolved.contains("Actor.new()"));
}

#[test]
fn gdscript_resolves_only_explicitly_typed_receivers() {
    let fixture = TestDirectory::new("gdscript-typed-receivers");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "actor.gd",
        "class_name Actor\nfunc spawn():\n    pass\n",
    );
    write(
        &repo,
        "caller.gd",
        "class_name Caller\nvar member: Actor\nfunc run(argument: Actor):\n    var local: Actor\n    argument.spawn()\n    local.spawn()\n    member.spawn()\n",
    );
    write(
        &repo,
        "ambiguous_one.gd",
        "class_name Duplicate\nfunc spawn():\n    pass\n",
    );
    write(
        &repo,
        "ambiguous_two.gd",
        "class_name Duplicate\nfunc spawn():\n    pass\n",
    );
    write(
        &repo,
        "ambiguous_caller.gd",
        "func run(argument: Duplicate):\n    argument.spawn()\n    Node.spawn()\n    missing.spawn()\n    untyped.spawn()\n",
    );

    let analysis = analyze(&repo);
    let spawn = node_id(&analysis.records, "function", "spawn", "actor.gd");
    assert_eq!(
        edges(&analysis.records)
            .iter()
            .filter(|edge| edge.relation == "calls" && edge.target == spawn)
            .count(),
        3
    );
    let unresolved = unresolved(&analysis.records)
        .into_iter()
        .filter(|value| value.relation == "calls")
        .map(|value| value.expression.as_str())
        .collect::<BTreeSet<_>>();
    for expected in [
        "argument.spawn()",
        "Node.spawn()",
        "missing.spawn()",
        "untyped.spawn()",
    ] {
        assert!(
            unresolved.contains(expected),
            "missing {expected}: {unresolved:?}"
        );
    }
}

#[test]
fn gdscript_prefers_same_file_class_declarations() {
    let fixture = TestDirectory::new("gdscript-same-file");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "external.gd",
        "class_name Widget\nstatic func ping():\n    pass\n",
    );
    write(
        &repo,
        "caller.gd",
        "class_name Widget\nstatic func ping():\n    pass\nvar member: Widget\nfunc run():\n    Widget.ping()\n    Widget.new()\n    member.ping()\n",
    );
    write(
        &repo,
        "ambiguous.gd",
        "class_name Duplicate\nstatic func ping():\n    pass\n",
    );
    write(
        &repo,
        "ambiguous_caller.gd",
        "class_name Duplicate\nstatic func ping():\n    pass\nclass_name Duplicate\nstatic func ping():\n    pass\nvar member: Duplicate\nfunc run():\n    Duplicate.ping()\n    Duplicate.new()\n    member.ping()\n",
    );

    let analysis = analyze(&repo);
    let local_type = node_id(&analysis.records, "type", "Widget", "caller.gd");
    let local_ping = node_id(&analysis.records, "function", "ping", "caller.gd");
    let external_ping = node_id(&analysis.records, "function", "ping", "external.gd");
    let edges = edges(&analysis.records);
    let local_calls = edges
        .iter()
        .filter(|edge| edge.relation == "calls" && matches!(edge.target.as_str(), target if target == local_type || target == local_ping))
        .count();
    let external_calls = edges
        .iter()
        .filter(|edge| edge.relation == "calls" && edge.target == external_ping)
        .count();
    assert_eq!(local_calls, 3);
    assert_eq!(external_calls, 0);

    let unresolved = unresolved(&analysis.records)
        .into_iter()
        .filter(|value| value.relation == "calls")
        .map(|value| value.expression.as_str())
        .collect::<BTreeSet<_>>();
    for expected in ["Duplicate.ping()", "Duplicate.new()", "member.ping()"] {
        assert!(
            unresolved.contains(expected),
            "missing ambiguous {expected}"
        );
    }
}

#[test]
fn gdscript_preload_aliases_are_resolved_conservatively() {
    let fixture = TestDirectory::new("gdscript-preload");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "scripts/tool.gd",
        "static func build():\n    pass\nfunc instance_method():\n    pass\n",
    );
    write(&repo, "scripts/typed_tool.gd", "class_name TypedTool\n");
    write(
        &repo,
        "scripts/ambiguous.gd",
        "class_name Duplicate\nclass_name Duplicate\n",
    );
    write(
        &repo,
        "caller.gd",
        "const Tool = preload(\"res://scripts/tool.gd\")\nconst TypedAlias = preload(\"res://scripts/typed_tool.gd\")\nconst Missing = preload(\"res://scripts/missing.gd\")\nconst Dynamic = preload(script_path)\nconst Loaded = load(\"res://scripts/tool.gd\")\nconst ResourceAlias = preload(\"res://data/resource.tres\")\nconst Ambiguous = preload(\"res://scripts/ambiguous.gd\")\nfunc run():\n    Tool.new()\n    Tool.build()\n    Tool.instance_method()\n    Tool.missing()\n    TypedAlias.new()\n    Missing.new()\n    Dynamic.new()\n    Loaded.new()\n    ResourceAlias.new()\n    Ambiguous.new()\n",
    );
    let analysis = analyze(&repo);
    let tool_module = node_id(&analysis.records, "module", "tool", "scripts/tool.gd");
    let build = node_id(&analysis.records, "function", "build", "scripts/tool.gd");
    let typed_tool = node_id(
        &analysis.records,
        "type",
        "TypedTool",
        "scripts/typed_tool.gd",
    );
    let call_edges = edges(&analysis.records)
        .into_iter()
        .filter(|edge| edge.relation == "calls")
        .collect::<Vec<_>>();
    assert_eq!(
        call_edges
            .iter()
            .filter(|edge| edge.target == tool_module)
            .count(),
        2
    );
    assert_eq!(
        call_edges
            .iter()
            .filter(|edge| edge.target == build)
            .count(),
        1
    );
    assert_eq!(
        call_edges
            .iter()
            .filter(|edge| edge.target == typed_tool)
            .count(),
        1
    );

    let unresolved = unresolved(&analysis.records)
        .into_iter()
        .filter(|value| value.relation == "calls")
        .map(|value| value.expression.as_str())
        .collect::<BTreeSet<_>>();
    for expected in [
        "Tool.instance_method()",
        "Tool.missing()",
        "Missing.new()",
        "Dynamic.new()",
        "ResourceAlias.new()",
        "Ambiguous.new()",
    ] {
        assert!(
            unresolved.contains(expected),
            "missing {expected}: {unresolved:?}"
        );
    }
}

#[test]
fn gdscript_resolves_inheritance_self_super_and_overrides() {
    let fixture = TestDirectory::new("gdscript-inheritance");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "base.gd",
        "class_name Base\nfunc _init():\n    pass\nfunc ping():\n    pass\n",
    );
    write(
        &repo,
        "child.gd",
        "class_name Child extends Base\nfunc ping():\n    pass\nfunc run():\n    ping()\n    self.ping()\n    super.ping()\n    super()\n",
    );
    let analysis = analyze(&repo);
    let base_ping = node_id(&analysis.records, "function", "ping", "base.gd");
    let child_ping = node_id(&analysis.records, "function", "ping", "child.gd");
    let init = node_id(&analysis.records, "function", "_init", "base.gd");
    let run = node_id(&analysis.records, "function", "run", "child.gd");
    assert!(has_edge(
        &analysis.records,
        "overrides",
        &child_ping,
        &base_ping
    ));
    assert!(has_edge(&analysis.records, "calls", &run, &child_ping));
    assert!(has_edge(&analysis.records, "calls", &run, &base_ping));
    assert!(has_edge(&analysis.records, "calls", &run, &init));
}

#[test]
fn gdscript_polymorphic_dispatch_keeps_possible_targets_and_exact_receivers_narrow() {
    let fixture = TestDirectory::new("gdscript-polymorphic");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "base.gd",
        "class_name Base\nfunc ping():\n    pass\n",
    );
    write(
        &repo,
        "child.gd",
        "class_name Child extends Base\nfunc ping():\n    pass\n",
    );
    write(
        &repo,
        "caller.gd",
        "func dispatch(value: Base):\n    value.ping()\nfunc exact():\n    Child.new().ping()\nfunc dynamic(value, method_name):\n    value.call(method_name)\n",
    );
    let analysis = analyze(&repo);
    let base_ping = node_id(&analysis.records, "function", "ping", "base.gd");
    let child_ping = node_id(&analysis.records, "function", "ping", "child.gd");
    let dispatch = node_id(&analysis.records, "function", "dispatch", "caller.gd");
    let exact = node_id(&analysis.records, "function", "exact", "caller.gd");
    assert!(has_edge(
        &analysis.records,
        "possible-calls",
        &dispatch,
        &base_ping
    ));
    assert!(has_edge(
        &analysis.records,
        "possible-calls",
        &dispatch,
        &child_ping
    ));
    assert!(has_edge(&analysis.records, "calls", &exact, &child_ping));
    assert!(unresolved(&analysis.records).iter().any(|value| {
        value.relation == "calls"
            && value.expression.contains("value.call")
            && value.reason == "dynamic-target"
    }));
}

#[test]
fn gdscript_propagates_constructor_arguments_assignments_and_returns() {
    let fixture = TestDirectory::new("gdscript-flow-propagation");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "dependency.gd",
        "class_name Dependency\nfunc work():\n    pass\n",
    );
    write(
        &repo,
        "factory.gd",
        "class_name Factory\nstatic func make() -> Dependency:\n    return Dependency.new()\n",
    );
    write(
        &repo,
        "consumer.gd",
        "class_name Consumer\nvar dependency\nfunc configure(value):\n    dependency = value\nfunc run():\n    dependency.work()\n",
    );
    write(
        &repo,
        "caller.gd",
        "func execute():\n    var dependency := Factory.make()\n    var consumer := Consumer.new()\n    consumer.configure(dependency)\n    consumer.run()\n    Factory.make().work()\n",
    );
    let analysis = analyze(&repo);
    let work = node_id(&analysis.records, "function", "work", "dependency.gd");
    let configure = node_id(&analysis.records, "function", "configure", "consumer.gd");
    let run = node_id(&analysis.records, "function", "run", "consumer.gd");
    assert_eq!(count_target_edges(&analysis.records, "calls", &work), 2);
    assert!(count_target_edges(&analysis.records, "calls", &configure) > 0);
    assert!(count_target_edges(&analysis.records, "calls", &run) > 0);
}

#[test]
fn gdscript_keeps_inner_class_and_lambda_ownership() {
    let fixture = TestDirectory::new("gdscript-inner-lambda");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "nested.gd",
        "class Inner:\n    func ping():\n        pass\n    func run():\n        ping()\nfunc outer():\n    Inner.new().run()\n",
    );
    write(
        &repo,
        "service.gd",
        "class_name LambdaService\nfunc ping():\n    pass\n",
    );
    write(
        &repo,
        "lambda.gd",
        "class_name LambdaOwner\nvar service := LambdaService.new()\nfunc wire(signal_value: Signal):\n    signal_value.connect(\n        func():\n            service.ping()\n    )\n",
    );
    let analysis = analyze(&repo);
    let inner_ping = node_id(&analysis.records, "function", "ping", "nested.gd");
    let inner_run = node_id(&analysis.records, "function", "run", "nested.gd");
    assert_eq!(
        count_target_edges(&analysis.records, "calls", &inner_ping),
        1
    );
    assert_eq!(
        count_target_edges(&analysis.records, "calls", &inner_run),
        1
    );

    let service_ping = node_id(&analysis.records, "function", "ping", "service.gd");
    let lambda = nodes(&analysis.records)
        .into_iter()
        .find(|node| node.kind == "function" && node.name.starts_with("<lambda@"))
        .expect("lambda node")
        .id
        .clone();
    assert!(has_edge(&analysis.records, "calls", &lambda, &service_ping));
    assert_eq!(
        count_target_edges(&analysis.records, "possible-calls", &lambda),
        1
    );
}

#[test]
fn gdscript_resolves_literal_load_preload_instances_and_nested_types() {
    let fixture = TestDirectory::new("gdscript-load-nested");
    let repo = fixture.path.join("repository");
    write(&repo, "loaded.gd", "func ping():\n    pass\n");
    write(
        &repo,
        "loaded_caller.gd",
        "static var Loaded = load(\"res://loaded.gd\")\nvar direct = load(\"res://loaded.gd\").new()\nfunc run():\n    var value = Loaded.new()\n    value.ping()\n    direct.ping()\n",
    );
    write(
        &repo,
        "tools.gd",
        "class Inner:\n    func run():\n        pass\nfunc instance_method():\n    pass\n",
    );
    write(
        &repo,
        "caller.gd",
        "const Tools = preload(\"res://tools.gd\")\nfunc execute():\n    var tool = Tools.new()\n    tool.instance_method()\n    Tools.Inner.new().run()\n    preload(\"res://tools.gd\").new().instance_method()\n",
    );
    let analysis = analyze(&repo);
    let ping = node_id(&analysis.records, "function", "ping", "loaded.gd");
    let instance_method = node_id(&analysis.records, "function", "instance_method", "tools.gd");
    let nested_run = nodes(&analysis.records)
        .into_iter()
        .find(|node| node.kind == "function" && node.name == "run" && node.path == "tools.gd")
        .unwrap()
        .id
        .clone();
    assert_eq!(count_target_edges(&analysis.records, "calls", &ping), 2);
    assert_eq!(
        count_target_edges(&analysis.records, "calls", &instance_method),
        2
    );
    assert_eq!(
        count_target_edges(&analysis.records, "calls", &nested_run),
        1
    );
}

#[test]
fn gdscript_resolves_configured_autoload_singletons() {
    let fixture = TestDirectory::new("gdscript-autoload");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "project.godot",
        "[autoload]\nLogger=\"*res://logger.gd\"\n",
    );
    write(
        &repo,
        "logger.gd",
        "class_name LocalLogger\nfunc write_message():\n    pass\n",
    );
    write(
        &repo,
        "caller.gd",
        "func run():\n    Logger.write_message()\n",
    );
    let analysis = analyze(&repo);
    let target = node_id(&analysis.records, "function", "write_message", "logger.gd");
    assert_eq!(count_target_edges(&analysis.records, "calls", &target), 1);
}

#[test]
fn gdscript_classifies_builtin_and_unknown_instance_methods() {
    let fixture = TestDirectory::new("gdscript-classification");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "builtin_receiver.gd",
        "class_name BuiltinReceiver\nvar control: Control\nfunc wire():\n    control.connect(\"ready\", Callable(self, \"wire\"))\n    control.has_signal(\"ready\")\n",
    );
    write(
        &repo,
        "worker.gd",
        "class_name Worker\nfunc run():\n    var child := Worker.new()\n    child.queue_free()\n",
    );
    let analysis = analyze(&repo);
    for expression in [
        "control.connect(\"ready\",Callable(self,\"wire\"))",
        "control.has_signal(\"ready\")",
    ] {
        assert!(
            unresolved(&analysis.records).iter().any(|value| {
                value.relation == "calls"
                    && value.expression == expression
                    && value.reason == "builtin-target"
            }),
            "missing builtin {expression}"
        );
    }
    assert!(unresolved(&analysis.records).iter().any(|value| {
        value.relation == "calls"
            && value.expression == "child.queue_free()"
            && value.reason == "external-target"
    }));
}

fn has_edge(records: &[FactRecord], relation: &str, source: &str, target: &str) -> bool {
    edges(records)
        .iter()
        .any(|edge| edge.relation == relation && edge.source == source && edge.target == target)
}

fn count_target_edges(records: &[FactRecord], relation: &str, target: &str) -> usize {
    edges(records)
        .iter()
        .filter(|edge| edge.relation == relation && edge.target == target)
        .count()
}

#[test]
fn gdscript_propagates_callable_properties_maps_and_arguments() {
    let fixture = TestDirectory::new("gdscript-callable-flow");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "session.gd",
        "class_name Session\nvar route: Callable\nfunc invoke():\n    route.call()\n",
    );
    write(
        &repo,
        "composer.gd",
        "class_name Composer\nfunc handle():\n    pass\nfunc configure(session: Session):\n    session.route = Callable(self, \"handle\")\n    session.invoke()\n",
    );
    write(
        &repo,
        "callback_map.gd",
        "class_name CallbackMap\nvar callbacks: Dictionary\nfunc configure(value: Dictionary):\n    callbacks = value\nfunc invoke():\n    var handler: Callable = callbacks.get(\"ready\", Callable())\n    handler.call()\nfunc ready():\n    pass\nfunc wire():\n    configure({\"ready\": Callable(self, \"ready\")})\n    invoke()\n",
    );
    write(
        &repo,
        "routes.gd",
        "class_name Routes\nvar route\nfunc configure(callback):\n    route = callback\nfunc invoke():\n    route.call()\nfunc handle():\n    pass\nfunc wire():\n    configure(Callable(self, \"handle\"))\n    invoke()\n",
    );

    let analysis = analyze(&repo);
    for (name, path) in [
        ("handle", "composer.gd"),
        ("ready", "callback_map.gd"),
        ("handle", "routes.gd"),
    ] {
        let target = node_id(&analysis.records, "function", name, path);
        assert_eq!(
            count_target_edges(&analysis.records, "calls", &target),
            1,
            "{path}::{name}"
        );
    }
}

#[test]
fn gdscript_emits_possible_callback_calls() {
    let fixture = TestDirectory::new("gdscript-callbacks");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "callbacks.gd",
        "class_name Callbacks\nfunc handle():\n    pass\nfunc wire(signal_value: Signal, values: Array):\n    var callback = Callable(self, \"handle\")\n    signal_value.connect(handle)\n    values.map(handle)\n",
    );
    let analysis = analyze(&repo);
    let handle = node_id(&analysis.records, "function", "handle", "callbacks.gd");
    assert!(
        count_target_edges(&analysis.records, "possible-calls", &handle) >= 3,
        "callback references were not emitted"
    );
}

#[test]
fn gdscript_analysis_is_deterministic() {
    let fixture = TestDirectory::new("gdscript-deterministic");
    let repo = fixture.path.join("repository");
    write(&repo, "b.gd", "func z():\n    pass\n");
    write(&repo, "a.gd", "func a():\n    z()\n");
    let first = analyze(&repo);
    let second = analyze(&repo);
    assert_eq!(first, second);
    assert!(
        nodes(&first.records)
            .iter()
            .all(|node| node.id.starts_with("sha256:"))
    );
}

#[test]
fn gdscript_scopes_nested_projects_and_autoloads() {
    let fixture = TestDirectory::new("gdscript-nested-projects");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "game_a/project.godot",
        "[autoload]\nService=\"*res://scripts/service.gd\"\n",
    );
    write(
        &repo,
        "game_a/scripts/base.gd",
        "func base_call():\n    pass\n",
    );
    write(
        &repo,
        "game_a/scripts/service.gd",
        "func run():\n    pass\n",
    );
    write(
        &repo,
        "game_a/scripts/caller.gd",
        "extends \"res://scripts/base.gd\"\nconst ServiceScript := preload(\"res://scripts/service.gd\")\nfunc call_service():\n    var instance := ServiceScript.new()\n    instance.run()\n    Service.run()\n",
    );
    write(
        &repo,
        "game_b/project.godot",
        "[autoload]\nService=\"*res://scripts/service.gd\"\n",
    );
    write(
        &repo,
        "game_b/scripts/service.gd",
        "func run():\n    pass\n",
    );
    write(
        &repo,
        "game_b/scripts/caller.gd",
        "func call_service():\n    Service.run()\n",
    );

    let analysis = analyze(&repo);
    let caller_a = node_id(
        &analysis.records,
        "function",
        "call_service",
        "game_a/scripts/caller.gd",
    );
    let caller_b = node_id(
        &analysis.records,
        "function",
        "call_service",
        "game_b/scripts/caller.gd",
    );
    let service_a = node_id(
        &analysis.records,
        "function",
        "run",
        "game_a/scripts/service.gd",
    );
    let service_b = node_id(
        &analysis.records,
        "function",
        "run",
        "game_b/scripts/service.gd",
    );
    let base_a = node_id(
        &analysis.records,
        "module",
        "base",
        "game_a/scripts/base.gd",
    );
    let caller_module_a = node_id(
        &analysis.records,
        "module",
        "caller",
        "game_a/scripts/caller.gd",
    );

    assert!(has_edge(&analysis.records, "calls", &caller_a, &service_a));
    assert!(!has_edge(&analysis.records, "calls", &caller_a, &service_b));
    assert!(has_edge(&analysis.records, "calls", &caller_b, &service_b));
    assert!(!has_edge(&analysis.records, "calls", &caller_b, &service_a));
    assert!(has_edge(
        &analysis.records,
        "extends",
        &caller_module_a,
        &base_a
    ));
}

#[test]
fn gdscript_file_preload_shadows_same_named_autoload() {
    let fixture = TestDirectory::new("gdscript-preload-shadow");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "project.godot",
        "[autoload]\nService=\"*res://autoload_service.gd\"\n",
    );
    write(&repo, "autoload_service.gd", "func run():\n    pass\n");
    write(&repo, "local_service.gd", "func run():\n    pass\n");
    write(
        &repo,
        "caller.gd",
        "const Service := preload(\"res://local_service.gd\")\nfunc call_service():\n    var instance := Service.new()\n    instance.run()\n",
    );
    let analysis = analyze(&repo);
    let caller = node_id(&analysis.records, "function", "call_service", "caller.gd");
    let local = node_id(&analysis.records, "function", "run", "local_service.gd");
    let autoload = node_id(&analysis.records, "function", "run", "autoload_service.gd");
    assert!(has_edge(&analysis.records, "calls", &caller, &local));
    assert!(!has_edge(&analysis.records, "calls", &caller, &autoload));
}

#[test]
fn gdscript_resolves_nested_type_aliases_from_preloaded_scripts() {
    let fixture = TestDirectory::new("gdscript-nested-alias");
    let repo = fixture.path.join("repository");
    write(&repo, "base.gd", "func configure():\n    pass\n");
    write(
        &repo,
        "support.gd",
        "const Base := preload(\"res://base.gd\")\nclass Nested extends Base:\n    pass\n",
    );
    write(
        &repo,
        "caller.gd",
        "const Support := preload(\"res://support.gd\")\nconst Alias := Support.Nested\nfunc run():\n    var value := Alias.new()\n    value.configure()\n",
    );
    let analysis = analyze(&repo);
    let run = node_id(&analysis.records, "function", "run", "caller.gd");
    let configure = node_id(&analysis.records, "function", "configure", "base.gd");
    let nested = node_id(&analysis.records, "type", "Nested", "support.gd");
    assert!(has_edge(&analysis.records, "calls", &run, &nested));
    assert!(has_edge(&analysis.records, "calls", &run, &configure));
}

#[test]
fn gdscript_resolves_members_after_chained_call_results() {
    let fixture = TestDirectory::new("gdscript-chained-results");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "chain.gd",
        "class Tracker:\n    func needs_resync():\n        pass\nclass Router:\n    var tracker: Tracker\nclass Pipeline:\n    func get_router() -> Router:\n        return Router.new()\nfunc run():\n    var pipeline := Pipeline.new()\n    pipeline.get_router().tracker.needs_resync()\n",
    );
    let analysis = analyze(&repo);
    let run = node_id(&analysis.records, "function", "run", "chain.gd");
    let get_router = node_id(&analysis.records, "function", "get_router", "chain.gd");
    let needs_resync = node_id(&analysis.records, "function", "needs_resync", "chain.gd");
    assert!(has_edge(&analysis.records, "calls", &run, &get_router));
    assert!(has_edge(&analysis.records, "calls", &run, &needs_resync));
}

#[test]
fn gdscript_keeps_immediate_receivers_across_expression_boundaries() {
    let fixture = TestDirectory::new("gdscript-expression-boundaries");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "service.gd",
        "class_name Service\nstatic func static_ready() -> bool:\n    return true\nfunc ready() -> bool:\n    return true\nfunc stop():\n    pass\n",
    );
    write(
        &repo,
        "caller.gd",
        "func run():\n    var service := Service.new()\n    if !service.ready():\n        return\n    var ready := 1 + Service.static_ready()\n    service.ready(); service.stop()\n",
    );
    let analysis = analyze(&repo);
    let run = node_id(&analysis.records, "function", "run", "caller.gd");
    for target in [
        node_id(&analysis.records, "function", "ready", "service.gd"),
        node_id(&analysis.records, "function", "static_ready", "service.gd"),
        node_id(&analysis.records, "function", "stop", "service.gd"),
    ] {
        assert!(
            has_edge(&analysis.records, "calls", &run, &target),
            "missing call target {target}"
        );
    }
}

#[test]
fn gdscript_dataflow_uses_nearest_prior_shadowed_local() {
    let fixture = TestDirectory::new("gdscript-shadowed-local");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "fixture.gd",
        "func run(value):\n    print(value)\n    var value = 1\n    print(value)\n    var value = value\n    print(value)\n",
    );
    let analysis = analyze(&repo);
    let run = node_id(&analysis.records, "function", "run", "fixture.gd");
    let parameter = nodes(&analysis.records)
        .into_iter()
        .find(|node| node.kind == "parameter" && node.name == "value")
        .unwrap()
        .id
        .clone();
    let mut locals = nodes(&analysis.records)
        .into_iter()
        .filter(|node| node.kind == "variable" && node.name == "value")
        .map(|node| (node.span.as_ref().unwrap().start_line, node.id.clone()))
        .collect::<Vec<_>>();
    locals.sort();
    assert_eq!(locals.len(), 2);

    let reads = edges(&analysis.records)
        .into_iter()
        .filter(|edge| edge.source == run && edge.relation == "reads")
        .map(|edge| {
            (
                edge.span.as_ref().map(|span| span.start_line).unwrap_or(0),
                edge.target.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        reads
            .iter()
            .any(|(line, target)| *line == 2 && target == &parameter)
    );
    assert!(
        reads
            .iter()
            .any(|(line, target)| *line == 4 && target == &locals[0].1)
    );
    assert!(
        reads
            .iter()
            .any(|(line, target)| *line == 5 && target == &locals[0].1)
    );
    assert!(
        reads
            .iter()
            .any(|(line, target)| *line == 6 && target == &locals[1].1)
    );
}

#[test]
fn gdscript_dataflow_rejects_ambiguous_member_owners() {
    let fixture = TestDirectory::new("gdscript-ambiguous-member");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "fixture.gd",
        "class First:\n    var session\nclass Second:\n    var session\nfunc run(flag):\n    var controller = First.new() if flag else Second.new()\n    controller.session = 1\n",
    );
    let analysis = analyze(&repo);
    let run = node_id(&analysis.records, "function", "run", "fixture.gd");
    let sessions = nodes(&analysis.records)
        .into_iter()
        .filter(|node| node.kind == "variable" && node.name == "session")
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(sessions.len(), 2);
    assert!(!edges(&analysis.records).iter().any(|edge| {
        edge.source == run && edge.relation == "writes" && sessions.contains(&edge.target)
    }));
}

#[test]
fn gdscript_unterminated_calls_are_not_emitted() {
    let fixture = TestDirectory::new("gdscript-unterminated-call");
    let repo = fixture.path.join("repository");
    write(&repo, "sample.gd", "var value = factory(\n");
    let analysis = analyze(&repo);
    assert!(
        !edges(&analysis.records)
            .iter()
            .any(|edge| matches!(edge.relation.as_str(), "calls" | "possible-calls"))
    );
    assert!(
        !unresolved(&analysis.records)
            .iter()
            .any(|value| value.relation == "calls")
    );
}

#[test]
fn gdscript_fact_emission_deduplicates_edges_and_unresolved_records() {
    let fixture = TestDirectory::new("gdscript-dedup");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "sample.gd",
        "class_name Sample\nfunc target():\n    pass\nfunc run():\n    target()\n    target()\n    missing.call()\n    missing.call()\n",
    );
    let analysis = analyze(&repo);

    let mut edge_keys = BTreeSet::new();
    for edge in edges(&analysis.records) {
        let key = format!(
            "{}|{}|{}|{:?}|{}",
            edge.source,
            edge.target,
            edge.relation,
            edge.span,
            edge.attributes
                .as_ref()
                .map_or(String::new(), serde_json::Value::to_string)
        );
        assert!(edge_keys.insert(key), "duplicate edge record");
    }
    let mut unresolved_keys = BTreeSet::new();
    for value in unresolved(&analysis.records) {
        let key = format!(
            "{}|{}|{}|{}|{:?}",
            value.source, value.relation, value.expression, value.reason, value.span
        );
        assert!(unresolved_keys.insert(key), "duplicate unresolved record");
    }
}

fn analyze(repo: &Path) -> lexicon::Analysis {
    GdscriptAdapter
        .analyze(&AdapterRequest {
            language: "gdscript".into(),
            repository: repo.to_path_buf(),
            ..Default::default()
        })
        .unwrap()
}

fn nodes(records: &[FactRecord]) -> Vec<&lexicon::NodeRecord> {
    records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(value) => Some(value),
            _ => None,
        })
        .collect()
}

fn edges(records: &[FactRecord]) -> Vec<&lexicon::EdgeRecord> {
    records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(value) => Some(value),
            _ => None,
        })
        .collect()
}

fn unresolved(records: &[FactRecord]) -> Vec<&lexicon::UnresolvedRecord> {
    records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Unresolved(value) => Some(value),
            _ => None,
        })
        .collect()
}

fn node_id(records: &[FactRecord], kind: &str, name: &str, path: &str) -> String {
    records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node)
                if node.kind == kind && node.name == name && node.path == path =>
            {
                Some(node.id.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {kind} {name} at {path}"))
}

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
