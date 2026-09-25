mod support;

use std::fs;
use std::path::Path;

use base64::Engine;
use lexicon::adapters::lotusscript::LotusScriptAdapter;
use lexicon::{AdapterHost, AdapterRequest, FactRecord, LanguageAdapter};

use support::TestDirectory;

#[test]
fn lotusscript_is_registered_by_default_with_native_fingerprint() {
    let fixture = TestDirectory::new("lotusscript-host");
    let host = AdapterHost::new(fixture.path.join("adapters"));
    assert!(host.has_adapter("lotusscript"));
    assert!(
        host.fingerprint("lotusscript")
            .unwrap()
            .starts_with("sha256:")
    );
}

#[test]
fn lotusscript_extracts_foundation_scope_inheritance_and_calls() {
    let fixture = TestDirectory::new("lotusscript-foundation");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "Core.lss",
        "Option Public\nPublic Class BaseWorker\n    Public Sub Run()\n        MsgBox(\"base\")\n    End Sub\nEnd Class\n\nPublic Function Helper(value As String) As String\n    Helper = UCase(value)\nEnd Function\n",
    );
    write(
        &repo,
        "agents/Worker.lss",
        "Option Declare\nUse \"Core\"\nPublic Class Worker As BaseWorker\n    Private mName As String\n    Public Sub New(name As String)\n        mName = name\n    End Sub\n    Public Sub Execute()\n        Call Helper(mName)\n        Call Me.Run()\n        session.GetDatabase(\"\", \"\")\n    End Sub\nEnd Class\n",
    );

    let analysis = analyze(&repo);
    for (kind, name) in [
        ("type", "Worker"),
        ("constructor", "New"),
        ("method", "Execute"),
        ("field", "mName"),
        ("parameter", "name"),
    ] {
        assert!(
            node(&analysis.records, kind, name, None).is_some(),
            "missing {kind} {name}"
        );
    }
    assert_relation(
        &analysis.records,
        "imports",
        "agents/Worker.lss",
        "Core",
        "Core.lss",
        "Core",
    );
    assert_relation(
        &analysis.records,
        "extends",
        "agents/Worker.lss",
        "Worker",
        "Core.lss",
        "BaseWorker",
    );
    assert_relation(
        &analysis.records,
        "calls",
        "agents/Worker.lss",
        "Execute",
        "Core.lss",
        "Helper",
    );
    assert_relation(
        &analysis.records,
        "calls",
        "agents/Worker.lss",
        "Execute",
        "Core.lss",
        "Run",
    );
    assert!(unresolved(
        &analysis.records,
        "calls",
        "builtin-target",
        "MsgBox"
    ));
    assert!(unresolved(
        &analysis.records,
        "calls",
        "dynamic-target",
        "session.GetDatabase"
    ));
}

#[test]
fn lotusscript_import_visibility_and_transitive_scope_match_oracle() {
    let fixture = TestDirectory::new("lotusscript-scope");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "Core.lss",
        "Public Sub CoreHelper()\nEnd Sub\nPrivate Sub Hidden()\nEnd Sub\n",
    );
    write(
        &repo,
        "Facade.lss",
        "Use \"Core\"\nPublic Sub FacadeHelper()\nEnd Sub\n",
    );
    write(
        &repo,
        "Caller.lss",
        "Use \"Facade\"\nPublic Sub Run()\n    Call CoreHelper()\n    Call Hidden()\nEnd Sub\n",
    );
    write(&repo, "Other.lss", "Public Sub CoreHelper()\nEnd Sub\n");

    let analysis = analyze(&repo);
    assert_relation(
        &analysis.records,
        "calls",
        "Caller.lss",
        "Run",
        "Core.lss",
        "CoreHelper",
    );
    assert_no_relation(
        &analysis.records,
        "calls",
        "Caller.lss",
        "Run",
        "Other.lss",
        "CoreHelper",
    );
    assert!(unresolved(
        &analysis.records,
        "calls",
        "external-target",
        "Hidden"
    ));
    assert!(!unresolved_any(&analysis.records, "CoreHelper"));
}

#[test]
fn lotusscript_option_public_exposes_default_declarations() {
    let fixture = TestDirectory::new("lotusscript-option-public");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "Library.lss",
        "Option Public\nSub VisibleByDefault()\nEnd Sub\n",
    );
    write(
        &repo,
        "Caller.lss",
        "Use \"Library\"\nPublic Sub Run()\n    Call VisibleByDefault()\nEnd Sub\n",
    );
    let analysis = analyze(&repo);
    assert_relation(
        &analysis.records,
        "calls",
        "Caller.lss",
        "Run",
        "Library.lss",
        "VisibleByDefault",
    );
}

#[test]
fn lotusscript_resolves_private_inherited_and_same_class_methods() {
    let fixture = TestDirectory::new("lotusscript-private-methods");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "Base.lss",
        "Option Public\nClass BaseWorker\n    Private Sub Hidden()\n    End Sub\nEnd Class\n",
    );
    write(
        &repo,
        "Derived.lss",
        "Use \"Base\"\nPublic Class Worker As BaseWorker\n    Public Sub Run()\n        Call Me.Hidden()\n    End Sub\nEnd Class\n",
    );
    write(
        &repo,
        "Same.lss",
        "Public Class Pair\n    Private Sub Secret()\n    End Sub\n    Public Sub Execute(other As Pair)\n        Call other.Secret()\n    End Sub\nEnd Class\n",
    );

    let analysis = analyze(&repo);
    assert_relation(
        &analysis.records,
        "calls",
        "Derived.lss",
        "Run",
        "Base.lss",
        "Hidden",
    );
    assert_relation(
        &analysis.records,
        "calls",
        "Same.lss",
        "Execute",
        "Same.lss",
        "Secret",
    );
}

#[test]
fn lotusscript_scopes_duplicate_class_names_and_typed_receivers() {
    let fixture = TestDirectory::new("lotusscript-typed");
    let repo = fixture.path.join("repository");
    for file in ["A.lss", "B.lss"] {
        write(
            &repo,
            file,
            "Public Class Worker\n    Public Sub New()\n    End Sub\n    Public Sub Run()\n    End Sub\nEnd Class\n",
        );
    }
    write(
        &repo,
        "Caller.lss",
        "Use \"A\"\nPublic Sub Execute()\n    Dim worker As New Worker\n    Call worker.Run()\nEnd Sub\n",
    );

    let analysis = analyze(&repo);
    assert_relation(
        &analysis.records,
        "calls",
        "Caller.lss",
        "Execute",
        "A.lss",
        "New",
    );
    assert_relation(
        &analysis.records,
        "calls",
        "Caller.lss",
        "Execute",
        "A.lss",
        "Run",
    );
    assert_no_relation(
        &analysis.records,
        "calls",
        "Caller.lss",
        "Execute",
        "B.lss",
        "New",
    );
    assert_no_relation(
        &analysis.records,
        "calls",
        "Caller.lss",
        "Execute",
        "B.lss",
        "Run",
    );
}

#[test]
fn lotusscript_with_colon_redim_and_indexing_match_oracle() {
    let fixture = TestDirectory::new("lotusscript-syntax");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "Library.lss",
        "Public Class Worker\n    Public Sub Run()\n    End Sub\nEnd Class\nPublic Sub First()\nEnd Sub\nPublic Sub Second(value As Variant)\nEnd Sub\nPublic Sub Execute()\n    Dim worker As New Worker\n    Static current As String\n    ReDim values(0 To 2)\n    With worker\n        Call .Run()\n    End With\n    Call First() : Call Second(#12:30:00#)\n    values(0) = current\nEnd Sub\n",
    );

    let analysis = analyze(&repo);
    assert_relation(
        &analysis.records,
        "calls",
        "Library.lss",
        "Execute",
        "Library.lss",
        "Run",
    );
    assert_relation(
        &analysis.records,
        "calls",
        "Library.lss",
        "Execute",
        "Library.lss",
        "First",
    );
    assert_relation(
        &analysis.records,
        "calls",
        "Library.lss",
        "Execute",
        "Library.lss",
        "Second",
    );
    assert!(node(&analysis.records, "variable", "current", None).is_some());
    assert!(node(&analysis.records, "variable", "values", None).is_some());
    assert!(!unresolved_any(&analysis.records, "values"));
}

#[test]
fn lotusscript_emits_conservative_variable_and_field_dataflow() {
    let fixture = TestDirectory::new("lotusscript-dataflow");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "Library.lss",
        "Public Class Counter\n    total As Integer\n    Public Sub Add(delta As Integer)\n        Me.total = Me.total + delta\n    End Sub\nEnd Class\nPublic shared As Integer\nPublic Sub Run()\n    Dim local As Integer\n    local = shared\n    shared = local\nEnd Sub\n",
    );

    let analysis = analyze(&repo);
    for (relation, source, target) in [
        ("writes", "Add", "total"),
        ("reads", "Add", "total"),
        ("reads", "Add", "delta"),
        ("writes", "Run", "local"),
        ("reads", "Run", "shared"),
        ("writes", "Run", "shared"),
        ("reads", "Run", "local"),
    ] {
        assert_relation(
            &analysis.records,
            relation,
            "Library.lss",
            source,
            "Library.lss",
            target,
        );
    }
}

#[test]
fn lotusscript_masks_strings_and_percent_rem_blocks() {
    let fixture = TestDirectory::new("lotusscript-comments");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "Library.lss",
        "Public Sub Run()\n%REM\n    Call Ghost()\n%END REM\n    Dim secret As String\n    Print \"secret\"\n    Call Real()\nEnd Sub\nPrivate Sub Real()\nEnd Sub\n",
    );
    let analysis = analyze(&repo);
    assert_relation(
        &analysis.records,
        "calls",
        "Library.lss",
        "Run",
        "Library.lss",
        "Real",
    );
    assert!(!unresolved_any(&analysis.records, "Ghost"));
    assert_no_relation(
        &analysis.records,
        "reads",
        "Library.lss",
        "Run",
        "Library.lss",
        "secret",
    );
}

#[test]
fn lotusscript_extracts_structured_and_raw_dxl_agents() {
    let fixture = TestDirectory::new("lotusscript-dxl");
    let repo = fixture.path.join("repository");
    let structured = "<?xml version='1.0'?><agent><code event='initialize'><lotusscript>Sub Initialize\n    Call ExecuteAgent()\nEnd Sub\nPrivate Sub ExecuteAgent\nEnd Sub</lotusscript></code></agent>";
    write(&repo, "Code/Agents/Structured.lsa", structured);

    let source = "'++LotusScript Development Environment:2:5:(Options):0:74\nOption Public\nOption Declare\nSub RawAgent\nEnd Sub\n";
    let mut payload = vec![0x81, 0x02, 0x85, 0xff, 0x20, 0, 0, 0];
    payload.extend_from_slice(source.as_bytes());
    payload.push(0);
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
    let raw = format!(
        "<?xml version='1.0'?><note><item name='$AssistAction'><rawitemdata type='10'>{encoded}</rawitemdata></item></note>"
    );
    write(&repo, "Code/Agents/Raw.lsa", &raw);

    let analysis = analyze(&repo);
    assert_relation(
        &analysis.records,
        "calls",
        "Code/Agents/Structured.lsa",
        "Initialize",
        "Code/Agents/Structured.lsa",
        "ExecuteAgent",
    );
    assert!(node(&analysis.records, "function", "RawAgent", None).is_some());
}

#[test]
fn lotusscript_analysis_is_deterministic() {
    let fixture = TestDirectory::new("lotusscript-deterministic");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "Library.ls",
        "Public Sub First()\n    Call Second()\nEnd Sub\nPrivate Sub Second()\nEnd Sub\n",
    );
    assert_eq!(analyze(&repo), analyze(&repo));
}

fn analyze(repo: &Path) -> lexicon::Analysis {
    let mut analysis = LotusScriptAdapter
        .analyze(&AdapterRequest {
            language: "lotusscript".into(),
            repository: repo.to_path_buf(),
            ..Default::default()
        })
        .unwrap();
    analysis.canonicalize().unwrap();
    analysis.validate().unwrap();
    analysis
}

fn node<'a>(
    records: &'a [FactRecord],
    kind: &str,
    name: &str,
    path: Option<&str>,
) -> Option<&'a lexicon::NodeRecord> {
    records.iter().find_map(|record| match record {
        FactRecord::Node(node)
            if node.kind == kind
                && node.name.eq_ignore_ascii_case(name)
                && path.is_none_or(|path| node.path == path) =>
        {
            Some(node)
        }
        _ => None,
    })
}

fn assert_relation(
    records: &[FactRecord],
    relation: &str,
    source_path: &str,
    source_name: &str,
    target_path: &str,
    target_name: &str,
) {
    let sources = node_ids(records, source_name, source_path);
    let targets = node_ids(records, target_name, target_path);
    assert!(
        !sources.is_empty(),
        "missing source {source_path}::{source_name}"
    );
    assert!(
        !targets.is_empty(),
        "missing target {target_path}::{target_name}"
    );
    assert!(
        records.iter().any(|record| match record {
            FactRecord::Edge(edge) if edge.relation == relation => {
                sources.contains(&edge.source) && targets.contains(&edge.target)
            }
            _ => false,
        }),
        "missing {relation}: {source_path}::{source_name} -> {target_path}::{target_name}"
    );
}

fn assert_no_relation(
    records: &[FactRecord],
    relation: &str,
    source_path: &str,
    source_name: &str,
    target_path: &str,
    target_name: &str,
) {
    let sources = node_ids(records, source_name, source_path);
    let targets = node_ids(records, target_name, target_path);
    assert!(!records.iter().any(|record| match record {
        FactRecord::Edge(edge) if edge.relation == relation => {
            sources.contains(&edge.source) && targets.contains(&edge.target)
        }
        _ => false,
    }));
}

fn node_ids(records: &[FactRecord], name: &str, path: &str) -> Vec<String> {
    records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) if node.path == path && node.name.eq_ignore_ascii_case(name) => {
                Some(node.id.clone())
            }
            _ => None,
        })
        .collect()
}

fn unresolved(records: &[FactRecord], relation: &str, reason: &str, candidate: &str) -> bool {
    records.iter().any(|record| match record {
        FactRecord::Unresolved(value) if value.relation == relation && value.reason == reason => {
            value
                .attributes
                .as_ref()
                .and_then(|attributes| attributes.get("candidate_name"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| name.eq_ignore_ascii_case(candidate))
        }
        _ => false,
    })
}

fn unresolved_any(records: &[FactRecord], candidate: &str) -> bool {
    records.iter().any(|record| match record {
        FactRecord::Unresolved(value) => value
            .attributes
            .as_ref()
            .and_then(|attributes| attributes.get("candidate_name"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|name| name.eq_ignore_ascii_case(candidate)),
        _ => false,
    })
}

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
