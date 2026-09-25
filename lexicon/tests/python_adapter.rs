mod support;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use lexicon::{AdapterHost, AdapterRequest, FactRecord, adapters::python::PythonAdapter};
use support::TestDirectory;

#[test]
fn python_declarations_imports_inheritance_and_nested_suffixes_resolve() {
    let root = TestDirectory::new("python-baseline");
    let repo = root.path.join("fixture");
    write(&repo, "pkg/__init__.py", "from .base import Base\n");
    write(
        &repo,
        "pkg/base.py",
        "class Base:\n    def run(self):\n        return 1\n",
    );
    write(
        &repo,
        "pkg/child.py",
        "from .base import Base\nclass Child(Base):\n    def run(self):\n        return Base()\n",
    );
    write(
        &repo,
        "tools/data_sync/config.py",
        "class NestedConfig:\n    pass\n",
    );
    write(
        &repo,
        "nested_consumer.py",
        "from data_sync.config import NestedConfig\ndef build():\n    return NestedConfig()\n",
    );
    write(&repo, "vendor/ignored.py", "class Ignored: pass\n");

    let analysis = analyze(&repo);
    let base = node(&analysis.records, "pkg.base.Base");
    let child = node(&analysis.records, "pkg.child.Child");
    let nested = node(&analysis.records, "tools.data_sync.config.NestedConfig");
    let build = node(&analysis.records, "nested_consumer.build");

    assert!(edge(&analysis.records, &child, &base, "extends"));
    assert!(edge(&analysis.records, &build, &nested, "calls"));
    assert!(analysis.records.iter().all(|record| match record {
        FactRecord::Node(node) => node.path != "vendor/ignored.py",
        _ => true,
    }));
}

#[test]
fn python_assignment_flow_branch_union_and_mro_resolve_calls() {
    let root = TestDirectory::new("python-flow");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "flow.py",
        "class Worker:\n    def run(self): return 1\n\nclass Alternate:\n    def run(self): return 2\n\nclass Left:\n    def pick(self): return 1\n\nclass Right:\n    def pick(self): return 2\n\nclass Child(Left, Right):\n    pass\n\ndef precise():\n    worker = Worker()\n    return worker.run()\n\ndef branch(flag):\n    worker = Worker()\n    if flag:\n        worker = Alternate()\n    return worker.run()\n\ndef inherited():\n    return Child().pick()\n",
    );

    let analysis = analyze(&repo);
    let worker = node(&analysis.records, "flow.Worker.run");
    let alternate = node(&analysis.records, "flow.Alternate.run");
    let left = node(&analysis.records, "flow.Left.pick");
    let precise = node(&analysis.records, "flow.precise");
    let branch = node(&analysis.records, "flow.branch");
    let inherited = node(&analysis.records, "flow.inherited");

    assert!(edge(&analysis.records, &precise, &worker, "calls"));
    assert!(edge(&analysis.records, &branch, &worker, "possible-calls"));
    assert!(edge(
        &analysis.records,
        &branch,
        &alternate,
        "possible-calls"
    ));
    assert!(edge(&analysis.records, &inherited, &left, "calls"));
}

#[test]
fn python_annotations_fields_and_polymorphic_dispatch_resolve() {
    let root = TestDirectory::new("python-annotations");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "typed.py",
        "class Base:\n    def run(self): return 1\n\nclass Child(Base):\n    def run(self): return 2\n\nclass Holder:\n    worker: Child\n    def use(self):\n        return self.worker.run()\n\ndef polymorphic(value: Base):\n    return value.run()\n",
    );

    let analysis = analyze(&repo);
    let base_run = node(&analysis.records, "typed.Base.run");
    let child_run = node(&analysis.records, "typed.Child.run");
    let holder_use = node(&analysis.records, "typed.Holder.use");
    let polymorphic = node(&analysis.records, "typed.polymorphic");

    assert!(edge(&analysis.records, &holder_use, &child_run, "calls"));
    assert!(edge(
        &analysis.records,
        &polymorphic,
        &base_run,
        "possible-calls"
    ));
    assert!(edge(
        &analysis.records,
        &polymorphic,
        &child_run,
        "possible-calls"
    ));
}

#[test]
fn python_semantic_error_contract_and_generated_suppression_match_oracle() {
    let root = TestDirectory::new("python-semantic-errors");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "sample.py",
        "import logging\n\ndef fallback():\n    return None\n\ndef swallowed():\n    try:\n        fallback()\n    except Exception:\n        pass\n\ndef propagated():\n    try:\n        fallback()\n    except Exception:\n        raise\n\ndef recorded():\n    try:\n        fallback()\n    except Exception as error:\n        logging.error('failed', error)\n\ndef recovered():\n    try:\n        fallback()\n    except Exception:\n        fallback()\n",
    );
    write(
        &repo,
        "generated.py",
        "# Code generated; DO NOT EDIT.\ntry:\n    work()\nexcept Exception:\n    pass\n",
    );

    let analysis = analyze(&repo);
    let semantic_nodes = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) if node.kind == "protocol" => Some(node),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(semantic_nodes.iter().any(|node| {
        node.name
            == "semantic-capabilities:python:control-flow,error-handling,calls,source-spans,outcome-obligations"
            && node.path == "sample.py"
    }));
    assert!(!semantic_nodes.iter().any(|node| {
        node.qualified_name.starts_with("@semantic/") && node.path == "generated.py"
    }));
    assert_eq!(
        semantic_nodes
            .iter()
            .filter(|node| node.name == "error-handler:python")
            .count(),
        4
    );
    for action in [
        "error-action:propagate",
        "error-action:record",
        "error-action:recover",
    ] {
        assert!(semantic_nodes.iter().any(|node| node.name == action));
    }
}

#[test]
fn python_empty_handler_error_flow_matches_oracle() {
    let root = TestDirectory::new("python-error-flow");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "sample.py",
        "def fallback_chain():\n    try:\n        import first\n    except ModuleNotFoundError:\n        pass\n    try:\n        import second\n    except ModuleNotFoundError:\n        pass\n    value = 1\n    return value\n\ndef cleanup():\n    try:\n        work()\n    except Exception:\n        if True:\n            try:\n                cleanup_work()\n            except OSError:\n                pass\n        raise\n\ndef nested_fallback():\n    if True:\n        try:\n            work()\n        except ValueError:\n            pass\n    return 2\n\ndef continued():\n    try:\n        work()\n    except Exception:\n        pass\n    record_original_error()\n",
    );

    let analysis = analyze(&repo);
    let mut flows = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) if node.name.starts_with("error-flow:") => {
                Some(node.name.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    flows.sort();
    let mut expected = vec![
        "error-flow:fallback".to_owned(),
        "error-flow:fallback".to_owned(),
        "error-flow:enclosing-propagation".to_owned(),
        "error-flow:fallback".to_owned(),
        "error-flow:continuation".to_owned(),
    ];
    expected.sort();
    assert_eq!(flows, expected);
}

#[test]
fn python_async_outcome_obligations_match_oracle() {
    let root = TestDirectory::new("python-outcomes");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "sample.py",
        "async def fetch():\n    return 1\n\nasync def use():\n    fetch()\n    await fetch()\n    value = fetch()\n    return fetch()\n",
    );

    let analysis = analyze(&repo);
    let operations = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) if node.name == "outcome-operation:python:async" => {
                Some(node.id.clone())
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let actions = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) if node.name == "outcome-action:consume" => {
                Some(node.id.clone())
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let consumed = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(edge)
                if edge.relation == "contains" && actions.contains(&edge.target) =>
            {
                Some(edge.source.clone())
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(operations.len(), 4);
    assert_eq!(actions.len(), 3);
    assert_eq!(operations.difference(&consumed).count(), 1);
}

#[test]
fn python_dependency_manifests_and_local_imports_match_oracle() {
    let root = TestDirectory::new("python-dependencies");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "pyproject.toml",
        "[project]\ndependencies = [\"requests>=2\", \"broken [\"]\n[project.optional-dependencies]\ntest = [\"pytest~=8\"]\n",
    );
    write(&repo, "pkg/local.py", "VALUE = 1\n");
    write(&repo, "dependency_user.py", "from pkg.local import VALUE\n");

    let analysis = analyze(&repo);
    let dependency_edges = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(edge) if edge.relation == "depends-on" => Some(edge),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(dependency_edges.len() >= 3);
    assert!(dependency_edges.iter().any(|edge| {
        edge.attributes
            .as_ref()
            .is_some_and(|value| value["category"] == "runtime" && value["constraint"] == ">=2")
    }));
    assert!(dependency_edges.iter().any(|edge| {
        edge.attributes
            .as_ref()
            .is_some_and(|value| value["category"] == "test" && value["dev"] == true)
    }));
    assert!(dependency_edges.iter().any(|edge| {
        edge.attributes
            .as_ref()
            .is_some_and(|value| value["category"] == "local" && value["path"] == true)
    }));
    assert!(!analysis.records.iter().any(|record| match record {
        FactRecord::Node(node) => node.qualified_name == "dependency:python:broken",
        _ => false,
    }));
}

#[test]
fn python_callbacks_closures_and_local_decorators_match_oracle() {
    let root = TestDirectory::new("python-callbacks");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "callbacks.py",
        "def alpha(): return 1\ndef beta(): return 2\n\ndef invoke(callback):\n    return callback()\n\ndef wrap(callback):\n    def inner():\n        return callback()\n    return inner\n\ndef decorate(func):\n    def wrapper():\n        return func()\n    return wrapper\n\n@decorate\ndef target():\n    return 1\n\ndef use():\n    invoke(alpha)\n    invoke(beta)\n    wrap(alpha)()\n    return target()\n",
    );

    let analysis = analyze(&repo);
    let alpha = node(&analysis.records, "callbacks.alpha");
    let beta = node(&analysis.records, "callbacks.beta");
    let invoke = node(&analysis.records, "callbacks.invoke");
    let inner = node(&analysis.records, "callbacks.wrap.inner");
    let wrapper = node(&analysis.records, "callbacks.decorate.wrapper");
    let target = node(&analysis.records, "callbacks.target");
    let use_id = node(&analysis.records, "callbacks.use");

    assert!(edge(&analysis.records, &invoke, &alpha, "possible-calls"));
    assert!(edge(&analysis.records, &invoke, &beta, "possible-calls"));
    assert!(
        edge(&analysis.records, &inner, &alpha, "calls")
            || edge(&analysis.records, &inner, &alpha, "possible-calls")
    );
    assert!(edge(&analysis.records, &use_id, &wrapper, "calls"));
    assert!(!edge(&analysis.records, &use_id, &target, "calls"));
}

#[test]
fn python_dataflow_reads_writes_and_shadowing_match_oracle() {
    let root = TestDirectory::new("python-dataflow");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "dataflow.py",
        "outer = 3\nclass Box:\n    field = 0\n    def run(self, value):\n        local = value\n        local += outer\n        self.field = local\n        def inner(value):\n            shadow = value\n            return shadow\n        callback = lambda item: item\n        return local + value + inner(value) + callback(value)\n",
    );

    let analysis = analyze(&repo);
    let run = node(&analysis.records, "dataflow.Box.run");
    let inner = node(&analysis.records, "dataflow.Box.run.inner");
    let nodes = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => Some((node.id.clone(), node)),
            _ => None,
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let flow = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(edge) if matches!(edge.relation.as_str(), "reads" | "writes") => {
                Some(edge)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    let run_names = flow
        .iter()
        .filter(|edge| edge.source == run)
        .filter_map(|edge| nodes.get(&edge.target).map(|node| node.name.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    let inner_names = flow
        .iter()
        .filter(|edge| edge.source == inner)
        .filter_map(|edge| nodes.get(&edge.target).map(|node| node.name.as_str()))
        .collect::<std::collections::BTreeSet<_>>();

    for expected in ["outer", "field", "value", "local"] {
        assert!(
            run_names.contains(expected),
            "run missing dataflow {expected}"
        );
    }
    assert!(inner_names.contains("value"));
    assert!(
        nodes
            .values()
            .any(|node| node.kind == "parameter" && node.name == "item")
    );
    assert!(!analysis.records.iter().any(|record| match record {
        FactRecord::Unresolved(value) => matches!(value.relation.as_str(), "reads" | "writes"),
        _ => false,
    }));
}

#[test]
fn python_transitive_reexports_and_nearest_modules_match_oracle() {
    let root = TestDirectory::new("python-reexports");
    let repo = root.path.join("fixture");
    write(&repo, "a.py", "from b import target\n");
    write(&repo, "b.py", "from c import target\n");
    write(&repo, "c.py", "from leaf import target\n");
    write(&repo, "leaf.py", "def target():\n    return 1\n");
    write(
        &repo,
        "use_reexport.py",
        "from a import target\n\ndef run():\n    return target()\n",
    );
    write(&repo, "suite/main.py", "def run():\n    return 1\n");
    write(&repo, "other/main.py", "def run():\n    return 2\n");
    write(
        &repo,
        "suite/tests/test_use.py",
        "from main import run\n\ndef use():\n    return run()\n",
    );

    let analysis = analyze(&repo);
    let use_reexport = node(&analysis.records, "use_reexport.run");
    let leaf = node(&analysis.records, "leaf.target");
    let use_nearest = node(&analysis.records, "suite.tests.test_use.use");
    let suite_run = node(&analysis.records, "suite.main.run");
    let other_run = node(&analysis.records, "other.main.run");

    assert!(edge(&analysis.records, &use_reexport, &leaf, "calls"));
    assert!(edge(&analysis.records, &use_nearest, &suite_run, "calls"));
    assert!(!edge(&analysis.records, &use_nearest, &other_run, "calls"));
}

#[test]
fn python_callable_values_getattr_partial_and_parametrize_match_oracle() {
    let root = TestDirectory::new("python-callable-values");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "callables.py",
        "import functools\nimport pytest\n\ndef alpha(value=None): return value\ndef beta(value=None): return value\n\nclass CallableThing:\n    def __call__(self, value=None): return value\n    def run(self): return 1\n\ndef invoke(callback):\n    return callback()\n\n@pytest.mark.parametrize('callback', [alpha, beta])\ndef parametrized(callback):\n    return callback()\n\ndef use():\n    invoke(alpha)\n    choices = [alpha, beta]\n    choices[0]()\n    thing = CallableThing()\n    thing()\n    bound = thing.run\n    bound()\n    getattr(thing, 'run')()\n    functools.partial(alpha)()\n",
    );

    let analysis = analyze(&repo);
    let alpha = node(&analysis.records, "callables.alpha");
    let beta = node(&analysis.records, "callables.beta");
    let invoke = node(&analysis.records, "callables.invoke");
    let parametrized = node(&analysis.records, "callables.parametrized");
    let use_id = node(&analysis.records, "callables.use");
    let call_method = node(&analysis.records, "callables.CallableThing.__call__");
    let run_method = node(&analysis.records, "callables.CallableThing.run");

    assert!(edge(&analysis.records, &invoke, &alpha, "calls"));
    assert!(edge(
        &analysis.records,
        &parametrized,
        &alpha,
        "possible-calls"
    ));
    assert!(edge(
        &analysis.records,
        &parametrized,
        &beta,
        "possible-calls"
    ));
    assert!(edge(&analysis.records, &use_id, &call_method, "calls"));
    assert!(edge(&analysis.records, &use_id, &run_method, "calls"));
    assert!(
        edge(&analysis.records, &use_id, &alpha, "calls")
            || edge(&analysis.records, &use_id, &alpha, "possible-calls")
    );
}

#[test]
fn python_runtime_target_classification_matches_oracle() {
    let root = TestDirectory::new("python-runtime-targets");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "runtime.py",
        "from pathlib import Path\nimport argparse\n\ndef use(values: list[str], root: Path):\n    sorted(values)\n    frozenset(values)\n    KeyError('missing')\n    values.append('x')\n    (root / 'file.txt').read_text(encoding='utf-8')\n    parser = argparse.ArgumentParser()\n    parser.parse_args([])\n\ndef dynamic(value, name):\n    return getattr(value, name)()\n",
    );

    let analysis = analyze(&repo);
    let unresolved = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Unresolved(value) if value.relation == "calls" => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let reasons = unresolved
        .iter()
        .map(|value| (value.expression.as_str(), value.reason.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();

    for expression in ["sorted(values)", "frozenset(values)", "KeyError('missing')"] {
        assert_eq!(
            reasons.get(expression),
            Some(&"builtin-target"),
            "{expression}"
        );
    }
    assert_eq!(
        reasons.get("getattr(value, name)()"),
        Some(&"dynamic-target")
    );
    assert!(
        unresolved
            .iter()
            .any(|value| value.reason == "external-target")
    );
}

#[test]
fn python_constructor_flow_remains_precise_and_conservative() {
    let root = TestDirectory::new("python-constructor-conservative");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "pkg/known.py",
        "class Worker:\n    def run(self): pass\n",
    );
    write(
        &repo,
        "pkg/other.py",
        "class Alternate:\n    def run(self): pass\n",
    );
    write(
        &repo,
        "left/config.py",
        "class Worker:\n    def run(self): pass\n",
    );
    write(
        &repo,
        "right/config.py",
        "class Worker:\n    def run(self): pass\n",
    );
    write(
        &repo,
        "local_precision.py",
        "from pkg.known import Worker\nfrom pkg.other import Alternate\nfrom config import Worker as AmbiguousWorker\n\ndef precise():\n    worker = Worker()\n    return worker.run()\n\ndef same_type_branch(flag):\n    worker = Worker()\n    if flag:\n        worker = Worker()\n    return worker.run()\n\ndef unconditional_reassignment():\n    worker = Worker()\n    worker = Alternate()\n    return worker.run()\n\ndef branch_union(flag):\n    worker = Worker()\n    if flag:\n        worker = Alternate()\n    return worker.run()\n\ndef attribute_based():\n    holder.worker = Worker()\n    return holder.worker.run()\n\ndef unknown_method():\n    worker = Worker()\n    return worker.missing()\n\ndef ambiguous_class():\n    worker = AmbiguousWorker()\n    return worker.run()\n",
    );

    let analysis = analyze(&repo);
    let worker = node(&analysis.records, "pkg.known.Worker.run");
    let alternate = node(&analysis.records, "pkg.other.Alternate.run");
    let precise = node(&analysis.records, "local_precision.precise");
    let same = node(&analysis.records, "local_precision.same_type_branch");
    let reassigned = node(
        &analysis.records,
        "local_precision.unconditional_reassignment",
    );
    let branch = node(&analysis.records, "local_precision.branch_union");
    let attribute = node(&analysis.records, "local_precision.attribute_based");
    let missing = node(&analysis.records, "local_precision.unknown_method");
    let ambiguous = node(&analysis.records, "local_precision.ambiguous_class");

    assert!(edge(&analysis.records, &precise, &worker, "calls"));
    assert!(edge(&analysis.records, &same, &worker, "calls"));
    assert!(edge(&analysis.records, &reassigned, &alternate, "calls"));
    assert!(edge(&analysis.records, &branch, &worker, "possible-calls"));
    assert!(edge(
        &analysis.records,
        &branch,
        &alternate,
        "possible-calls"
    ));
    for source in [&attribute, &missing, &ambiguous] {
        assert!(!edge(&analysis.records, source, &worker, "calls"));
        assert!(!edge(&analysis.records, source, &alternate, "calls"));
        assert!(!edge(&analysis.records, source, &worker, "possible-calls"));
        assert!(!edge(
            &analysis.records,
            source,
            &alternate,
            "possible-calls"
        ));
    }
    assert!(analysis.records.iter().any(|record| match record {
        FactRecord::Unresolved(value) =>
            value.source == missing
                && value.expression == "worker.missing()"
                && value.reason == "missing-target",
        _ => false,
    }));
}

#[test]
fn python_fields_factories_loops_and_nested_lexical_calls_match_oracle() {
    let root = TestDirectory::new("python-flow-complete");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "pkg/known.py",
        "class Worker:\n    def run(self): return 1\n",
    );
    write(
        &repo,
        "flow.py",
        "from pkg.known import Worker\n\nclass Holder:\n    worker: Worker\n    def use(self):\n        return self.worker.run()\n\nclass Builder:\n    @classmethod\n    def create(cls):\n        return cls()\n    def run(self):\n        return 1\n\ndef make_worker() -> Worker:\n    return Worker()\n\ndef consume(worker: Worker):\n    return worker.run()\n\ndef consume_many(workers: list[Worker]):\n    for worker in workers:\n        worker.run()\n    return [worker.run() for worker in workers]\n\ndef use():\n    make_worker().run()\n    consume(Worker())\n    consume_many([Worker()])\n    Builder.create().run()\n\ndef outer():\n    class Visitor:\n        def run(self):\n            return 1\n    def helper():\n        return Visitor().run()\n    return helper()\n",
    );
    write(
        &repo,
        "class_fields.py",
        "from pkg.known import Worker\n\nclass Direct:\n    worker = Worker()\n    def use(self):\n        return self.worker.run()\n\nclass Conditional:\n    if enabled:\n        worker = Worker()\n    def use(self):\n        return self.worker.run()\n",
    );

    let analysis = analyze(&repo);
    let worker_run = node(&analysis.records, "pkg.known.Worker.run");
    for source in [
        "flow.Holder.use",
        "flow.consume",
        "flow.consume_many",
        "flow.use",
        "class_fields.Direct.use",
    ] {
        let source_id = node(&analysis.records, source);
        assert!(
            edge(&analysis.records, &source_id, &worker_run, "calls")
                || edge(&analysis.records, &source_id, &worker_run, "possible-calls"),
            "{source} did not resolve Worker.run"
        );
    }

    let builder = node(&analysis.records, "flow.Builder");
    let builder_create = node(&analysis.records, "flow.Builder.create");
    let builder_run = node(&analysis.records, "flow.Builder.run");
    let use_id = node(&analysis.records, "flow.use");
    assert!(edge(&analysis.records, &builder_create, &builder, "calls"));
    assert!(edge(&analysis.records, &use_id, &builder_run, "calls"));

    let helper = node(&analysis.records, "flow.outer.helper");
    let visitor = node(&analysis.records, "flow.outer.Visitor");
    let visitor_run = node(&analysis.records, "flow.outer.Visitor.run");
    let outer = node(&analysis.records, "flow.outer");
    assert!(edge(&analysis.records, &helper, &visitor, "calls"));
    assert!(edge(&analysis.records, &helper, &visitor_run, "calls"));
    assert!(edge(&analysis.records, &outer, &helper, "calls"));

    let conditional = node(&analysis.records, "class_fields.Conditional.use");
    assert!(!edge(&analysis.records, &conditional, &worker_run, "calls"));
    assert!(!edge(
        &analysis.records,
        &conditional,
        &worker_run,
        "possible-calls"
    ));
    assert!(analysis.records.iter().any(|record| match record {
        FactRecord::Unresolved(value) =>
            value.source == conditional
                && value.expression == "self.worker.run()"
                && value.relation == "calls",
        _ => false,
    }));
}

#[test]
fn python_imported_callback_registry_matches_oracle() {
    let root = TestDirectory::new("python-imported-callbacks");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "pkg/generators.py",
        "def first(value):\n    return value\n\ndef second(value):\n    return value\n",
    );
    write(
        &repo,
        "callback_registry.py",
        "from typing import Callable\nfrom pkg import generators\n\nGenerator = Callable[[str], str]\nGENERATORS: dict[str, Generator] = {\n    'first': generators.first,\n    'second': generators.second,\n}\n\ndef use(name: str, value: str):\n    generator = GENERATORS.get(name)\n    if generator is None:\n        return value\n    return generator(value)\n",
    );

    let analysis = analyze(&repo);
    let use_id = node(&analysis.records, "callback_registry.use");
    let first = node(&analysis.records, "pkg.generators.first");
    let second = node(&analysis.records, "pkg.generators.second");
    assert!(edge(&analysis.records, &use_id, &first, "possible-calls"));
    assert!(edge(&analysis.records, &use_id, &second, "possible-calls"));
}

#[test]
fn python_protocol_dispatch_overrides_and_dynamic_fallback_match_oracle() {
    let root = TestDirectory::new("python-contract-dispatch");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "pkg/contracts.py",
        "from typing import Protocol\nclass Runner(Protocol):\n    def run(self): ...\nclass One(Runner):\n    def run(self): return 1\nclass Two(Runner):\n    def run(self): return 2\nclass Base:\n    def inherited(self): return 1\nclass Child(Base):\n    pass\ndef invoke(runner: Runner):\n    return runner.run()\ndef exact():\n    return One().run()\ndef inherited():\n    return Child().inherited()\ndef dynamic(value, name):\n    return getattr(value, name)()\n",
    );

    let analysis = analyze(&repo);
    let runner = node(&analysis.records, "pkg.contracts.Runner");
    let runner_run = node(&analysis.records, "pkg.contracts.Runner.run");
    let one = node(&analysis.records, "pkg.contracts.One");
    let two = node(&analysis.records, "pkg.contracts.Two");
    let one_run = node(&analysis.records, "pkg.contracts.One.run");
    let two_run = node(&analysis.records, "pkg.contracts.Two.run");
    let invoke = node(&analysis.records, "pkg.contracts.invoke");
    let exact = node(&analysis.records, "pkg.contracts.exact");
    let inherited = node(&analysis.records, "pkg.contracts.inherited");
    let base_inherited = node(&analysis.records, "pkg.contracts.Base.inherited");
    let child = node(&analysis.records, "pkg.contracts.Child");
    let base = node(&analysis.records, "pkg.contracts.Base");
    let dynamic = node(&analysis.records, "pkg.contracts.dynamic");

    assert!(edge(&analysis.records, &one, &runner, "implements"));
    assert!(edge(&analysis.records, &two, &runner, "implements"));
    assert!(edge(&analysis.records, &invoke, &one_run, "possible-calls"));
    assert!(edge(&analysis.records, &invoke, &two_run, "possible-calls"));
    assert!(edge(&analysis.records, &exact, &one_run, "calls"));
    assert!(edge(
        &analysis.records,
        &inherited,
        &base_inherited,
        "calls"
    ));
    assert!(edge(&analysis.records, &one_run, &runner_run, "overrides"));
    assert!(edge(&analysis.records, &two_run, &runner_run, "overrides"));
    assert!(edge(&analysis.records, &child, &base, "extends"));
    assert!(analysis.records.iter().any(|record| match record {
        FactRecord::Unresolved(value) =>
            value.source == dynamic && value.reason == "dynamic-target",
        _ => false,
    }));
}

#[test]
fn python_async_method_name_does_not_create_outcome_obligation() {
    let root = TestDirectory::new("python-outcome-boundary");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "sample.py",
        "class Worker:\n    async def run(self):\n        return 1\n\ndef use():\n    run()\n",
    );

    let analysis = analyze(&repo);
    assert!(!analysis.records.iter().any(|record| match record {
        FactRecord::Node(node) => node.name == "outcome-operation:python:async",
        _ => false,
    }));
}

#[test]
fn python_analysis_is_deterministic() {
    let root = TestDirectory::new("python-determinism");
    let repo = root.path.join("fixture");
    write(
        &repo,
        "main.py",
        "class Worker:\n    def run(self): return 1\ndef use():\n    return Worker().run()\n",
    );

    let first = analyze(&repo);
    let second = analyze(&repo);
    assert_eq!(first, second);
}

fn analyze(repository: &Path) -> lexicon::Analysis {
    let adapter_root = repository.parent().unwrap().join("adapters");
    fs::create_dir_all(&adapter_root).unwrap();
    let mut host = AdapterHost::new(adapter_root);
    host.register("python", Arc::new(PythonAdapter));
    host.analyze(&AdapterRequest {
        language: "python".into(),
        repository: repository.to_path_buf(),
        ..Default::default()
    })
    .unwrap()
}

fn node(records: &[FactRecord], qname: &str) -> String {
    records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.qualified_name == qname => Some(node.id.clone()),
            _ => None,
        })
        .unwrap_or_else(|| {
            let available = records
                .iter()
                .filter_map(|record| match record {
                    FactRecord::Node(node) => Some(node.qualified_name.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            panic!("missing node {qname}; available: {available:?}")
        })
}

fn edge(records: &[FactRecord], source: &str, target: &str, relation: &str) -> bool {
    records.iter().any(|record| match record {
        FactRecord::Edge(edge) => {
            edge.source == source && edge.target == target && edge.relation == relation
        }
        _ => false,
    })
}

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
