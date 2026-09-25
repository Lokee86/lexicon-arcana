use lexicon::{AdapterHost, AdapterRequest, FactRecord, NodeRecord};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn repository(files: &[(&str, &str)]) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("lexicon-ruby-{suffix}"));
    for (name, contents) in files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    root
}

fn analyze_root(root: &Path) -> Vec<FactRecord> {
    AdapterHost::new("adapters")
        .analyze(&AdapterRequest {
            language: "ruby".into(),
            repository: root.to_path_buf(),
            ..Default::default()
        })
        .unwrap()
        .records
}

fn analyze(files: &[(&str, &str)]) -> Vec<FactRecord> {
    let root = repository(files);
    let records = analyze_root(&root);
    let _ = fs::remove_dir_all(root);
    records
}

fn nodes(records: &[FactRecord]) -> BTreeMap<String, NodeRecord> {
    records
        .iter()
        .filter_map(|r| match r {
            FactRecord::Node(n) => Some((n.id.clone(), n.clone())),
            _ => None,
        })
        .collect()
}

fn id_for(nodes: &BTreeMap<String, NodeRecord>, qualified: &str) -> String {
    nodes
        .values()
        .find(|n| n.qualified_name == qualified)
        .unwrap_or_else(|| panic!("missing node {qualified}"))
        .id
        .clone()
}

fn call_targets(
    records: &[FactRecord],
    nodes: &BTreeMap<String, NodeRecord>,
    source: &str,
    relation: &str,
) -> Vec<String> {
    records
        .iter()
        .filter_map(|r| match r {
            FactRecord::Edge(e) if e.source == source && e.relation == relation => {
                nodes.get(&e.target).map(|n| n.qualified_name.clone())
            }
            _ => None,
        })
        .collect()
}

fn assert_call(
    records: &[FactRecord],
    nodes: &BTreeMap<String, NodeRecord>,
    source: &str,
    target: &str,
) {
    let source_id = id_for(nodes, source);
    assert!(
        call_targets(records, nodes, &source_id, "calls")
            .iter()
            .any(|q| q == target),
        "missing call {source} -> {target}\n{records:#?}"
    );
}

#[test]
fn extracts_declarations_imports_and_inheritance() {
    let records = analyze(&[
        (
            "lib/sample.rb",
            r#"require "json"
require(name)
module Outer
  VERSION = 1
  module Inner
    class Child < Base
      def run
      end
    end
  end
end
"#,
        ),
        ("lib/base.rb", "class Base; end\n"),
        ("vendor/ignored.rb", "class Ignored; end\n"),
        (".git/ignored.rb", "class IgnoredGit; end\n"),
    ]);
    let nodes = nodes(&records);
    for qualified in [
        "Outer",
        "Outer::VERSION",
        "Outer::Inner",
        "Outer::Inner::Child",
        "Outer::Inner::Child#run",
    ] {
        assert!(
            nodes.values().any(|n| n.qualified_name == qualified),
            "{qualified}"
        );
    }
    assert!(nodes.values().any(
        |n| n.kind == "import" && n.attributes.as_ref().is_some_and(|a| a["target"] == "json")
    ));
    for relation in ["contains", "defines", "imports", "extends"] {
        assert!(
            records
                .iter()
                .any(|r| matches!(r, FactRecord::Edge(e) if e.relation == relation))
        );
    }
    assert!(records.iter().any(|r| matches!(r, FactRecord::Unresolved(u)
        if u.relation == "imports" && u.reason == "dynamic-target")));
    assert!(
        !nodes
            .values()
            .any(|n| n.path.contains("vendor") || n.path.contains(".git"))
    );
}

#[test]
fn ids_and_output_are_deterministic() {
    let root = repository(&[
        ("lib/a.rb", "module A\n def x; end\nend\n"),
        ("lib/b.rb", "class B < A\nend\n"),
    ]);
    let first = analyze_root(&root);
    let second = analyze_root(&root);
    assert_eq!(first, second);
    let mut ids = first
        .iter()
        .filter_map(|r| match r {
            FactRecord::Node(n) => Some(n.id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let original_len = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), original_len);
    assert!(
        ids.iter()
            .all(|id| id.starts_with("sha256:") && id.len() == 71)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn emits_gemfile_gemspec_and_require_relative_dependencies() {
    let records = analyze(&[
        (
            "Gemfile",
            "source \"https://rubygems.org\"\ngem \"rails\", \"~> 7.0\"\ngroup :development do\n gem \"rubocop\", \">= 1\"\nend\n",
        ),
        (
            "demo.gemspec",
            "spec.add_dependency \"json\", \">= 2\"\nspec.add_development_dependency \"rspec\", \"~> 3\"\n",
        ),
        ("lib/main.rb", "require_relative \"support\"\n"),
        ("lib/support.rb", "VALUE = 1\n"),
        ("dynamic.rb", "require(name)\n"),
    ]);
    let deps = records
        .iter()
        .filter_map(|r| match r {
            FactRecord::Edge(e) if e.relation == "depends-on" => Some(e),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(deps.iter().any(|e| {
        e.attributes
            .as_ref()
            .is_some_and(|a| a["constraint"] == "~> 7.0")
    }));
    assert!(deps.iter().any(|e| {
        e.attributes
            .as_ref()
            .is_some_and(|a| a["category"] == "development" && a["dev"] == true)
    }));
    assert!(deps.iter().any(|e| {
        e.attributes
            .as_ref()
            .is_some_and(|a| a["category"] == "local" && a["path"] == "lib/support.rb")
    }));
    assert!(
        !nodes(&records)
            .values()
            .any(|n| n.qualified_name == "dependency:ruby:name")
    );
}

#[test]
fn preserves_callsites_blocks_and_ambiguity() {
    let records = analyze(&[(
        "local.rb",
        r#"class Local
  register :external_dsl

  def run
    helper
    unique(1)
    missing
    explicit.helper
    send(:helper)
    records.each do |record|
      helper
    end
    helper do
      helper
    end
  end

  def helper; end
  def unique(value); value; end
  def ambiguous; end
  def ambiguous; end
  def use_ambiguous
    ambiguous
  end
end
"#,
    )]);
    let nodes = nodes(&records);
    let run = id_for(&nodes, "Local#run");
    let direct = call_targets(&records, &nodes, &run, "calls");
    assert!(direct.iter().filter(|q| *q == "Local#helper").count() >= 2);
    assert!(direct.iter().any(|q| q == "Local#unique"));

    let block_ids = nodes
        .values()
        .filter(|n| {
            n.kind == "function" && n.attributes.as_ref().is_some_and(|a| a["block"] == true)
        })
        .map(|n| n.id.clone())
        .collect::<Vec<_>>();
    assert!(records.iter().any(|r| matches!(r, FactRecord::Edge(e)
        if block_ids.contains(&e.source)
        && nodes.get(&e.target).is_some_and(|n| n.qualified_name == "Local#helper"))));

    let unresolved = records
        .iter()
        .filter_map(|r| match r {
            FactRecord::Unresolved(u) => Some(u),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        unresolved
            .iter()
            .any(|u| u.expression == "missing" && u.reason == "missing-target")
    );
    assert!(
        unresolved
            .iter()
            .any(|u| u.expression == "explicit.helper" && u.reason == "dynamic-target")
    );
    assert!(
        unresolved
            .iter()
            .any(|u| u.relation == "defines" && u.expression.starts_with("send"))
    );
    let ambiguous_source = id_for(&nodes, "Local#use_ambiguous");
    assert!(
        call_targets(&records, &nodes, &ambiguous_source, "calls")
            .iter()
            .any(|q| q == "Local#ambiguous")
    );
    assert!(
        !unresolved
            .iter()
            .any(|u| u.source == ambiguous_source && u.reason == "ambiguous-target")
    );
}

#[test]
fn classifies_runtime_dispatch_without_false_static_ambiguity() {
    let records = analyze(&[(
        "runtime_dispatch.rb",
        r#"module FrameworkConcern
  def request_context
    request
    controller_path
  end
end

class HarnessOne
  include FrameworkConcern
  def request; end
  def controller_path; end
end

class HarnessTwo
  include FrameworkConcern
  def request; end
  def controller_path; end
end

class Around
  def call
    yield
  end
end

Around.new.call { :first }
Around.new.call { :second }

class Injected
  def initialize(clock)
    @clock = clock
  end

  def run
    @clock.call
  end
end

Injected.new(-> { 1 }).run
Injected.new(-> { 2 }).run
"#,
    )]);
    let nodes = nodes(&records);
    let concern = id_for(&nodes, "FrameworkConcern#request_context");
    for expr in ["request", "controller_path"] {
        assert!(records.iter().any(|r| matches!(r, FactRecord::Unresolved(u)
            if u.source == concern && u.expression == expr && u.reason == "external-target")));
    }
    assert!(!records.iter().any(|r| matches!(r, FactRecord::Edge(e)
        if e.source == concern && e.relation == "possible-calls")));

    let around = id_for(&nodes, "Around#call");
    let injected = id_for(&nodes, "Injected#run");
    for id in [&around, &injected] {
        assert!(records.iter().any(|r| matches!(r, FactRecord::Unresolved(u)
            if &u.source == id && u.reason == "dynamic-target")));
        assert!(
            !records.iter().any(|r| matches!(r, FactRecord::Unresolved(u)
            if &u.source == id && u.reason == "ambiguous-target"))
        );
    }
}

#[test]
fn excludes_complete_warlock_state_directory_set() {
    let dirs = [
        ".ddocs",
        ".lexicon",
        ".arcana",
        ".grimoire",
        ".pitlord",
        ".cantrip",
        ".homunculus",
        ".incubus",
        ".ritual",
        ".warlock",
    ];
    let files = dirs
        .iter()
        .map(|d| {
            (
                format!("{d}/ignored.rb"),
                "class IgnoredState; end\n".to_string(),
            )
        })
        .collect::<Vec<_>>();
    let refs = files
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect::<Vec<_>>();
    let records = analyze(&refs);
    let nodes = nodes(&records);
    for dir in dirs {
        assert!(
            !nodes
                .values()
                .any(|n| n.path == format!("{dir}/ignored.rb"))
        );
    }
}

#[test]
fn resolves_ruby_static_call_semantics() {
    let records = analyze(&[(
        "semantic.rb",
        r#"module Support
  def support_call
    base_helper
  end
end

module Installed
  def installed_call; end
end

class Framework; end
Framework.include(Installed)

class Base
  include Support
  def base_helper; end
  def inherited; end
end

class Product
  def initialize; end
  def work; end
end

module Factory
  module_function
  def build
    Product.new
  end
end

class SingletonFactory
  class << self
    def build
      Product.new
    end
  end
end

Result = Struct.new(:value) do
  def success?
    value
  end
end

class Child < Base
  def run
    support_call
    inherited
    Factory.build.work
    SingletonFactory.build.work
    result = Result.new(value: Product.new)
    result.success?
    around { base_helper }
  end

  def around
    yield
  end
end

class InstalledChild < Framework
  def run
    installed_call
  end
end

class Parent
  def execute; end
end

class SuperChild < Parent
  def execute
    super
  end
end

class Alpha
  def execute; end
end

class Beta
  def execute; end
end

def dispatch(value)
  value.execute
end

dispatch(Alpha.new)
dispatch(Beta.new)
"#,
    )]);
    let nodes = nodes(&records);
    for (source, target) in [
        ("Child#run", "Support#support_call"),
        ("Child#run", "Base#inherited"),
        ("Support#support_call", "Base#base_helper"),
        ("Child#run", "Factory.build"),
        ("Child#run", "SingletonFactory.build"),
        ("Child#run", "Result#success?"),
        ("Result#success?", "Result#value"),
        ("InstalledChild#run", "Installed#installed_call"),
        ("SuperChild#execute", "Parent#execute"),
    ] {
        assert_call(&records, &nodes, source, target);
    }
    let child = id_for(&nodes, "Child#run");
    assert!(
        call_targets(&records, &nodes, &child, "calls")
            .iter()
            .filter(|q| *q == "Product#work")
            .count()
            >= 2
    );

    let super_child = id_for(&nodes, "SuperChild#execute");
    let parent = id_for(&nodes, "Parent#execute");
    assert!(records.iter().any(|r| matches!(r, FactRecord::Edge(e)
        if e.source == super_child && e.target == parent && e.relation == "overrides")));

    let dispatch = id_for(&nodes, "dispatch");
    let mut possible = call_targets(&records, &nodes, &dispatch, "possible-calls");
    possible.sort();
    possible.dedup();
    assert_eq!(possible, vec!["Alpha#execute", "Beta#execute"]);
}

#[test]
fn emits_conservative_dataflow_for_reads_writes_compound_members_and_shadowing() {
    let records = analyze(&[(
        "dataflow.rb",
        r#"class Box
  VERSION = 3
  def run(value)
    @field = value
    local = value
    local += VERSION
    local += 1
    @field = local
    return local + value
  end
  def inner(value)
    local = value
    local
  end
end
"#,
    )]);
    let nodes = nodes(&records);
    let run = id_for(&nodes, "Box#run");
    let inner = id_for(&nodes, "Box#inner");
    let flow = records
        .iter()
        .filter_map(|r| match r {
            FactRecord::Edge(e) if e.relation == "reads" || e.relation == "writes" => Some(e),
            _ => None,
        })
        .collect::<Vec<_>>();

    let run_targets = flow
        .iter()
        .filter(|e| e.source == run)
        .filter_map(|e| nodes.get(&e.target))
        .collect::<Vec<_>>();
    let inner_targets = flow
        .iter()
        .filter(|e| e.source == inner)
        .filter_map(|e| nodes.get(&e.target))
        .collect::<Vec<_>>();
    assert!(
        run_targets
            .iter()
            .any(|n| n.name == "local" && n.kind == "variable")
    );
    assert!(
        run_targets
            .iter()
            .any(|n| n.name == "VERSION" && n.kind == "constant")
    );
    assert!(run_targets.iter().any(|n| n.name == "@field"));
    assert!(inner_targets.iter().any(|n| n.name == "value"));
    assert!(
        flow.iter()
            .any(|e| e.source == run && e.relation == "reads")
    );
    assert!(
        flow.iter()
            .any(|e| e.source == run && e.relation == "writes")
    );
    assert!(flow.iter().all(|e| nodes.contains_key(&e.target)));
    assert!(
        !records.iter().any(|r| matches!(r, FactRecord::Unresolved(u)
        if u.relation == "reads" || u.relation == "writes"))
    );
}
