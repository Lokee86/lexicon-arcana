mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use lexicon::adapters::generic::GenericAdapter;
use lexicon::{AdapterHost, AdapterMode, AdapterRequest, FactRecord, LanguageAdapter};

use support::TestDirectory;

#[test]
fn generic_adapter_emits_conservative_facts_and_excludes_noise() {
    let fixture = TestDirectory::new("generic-conservative");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "src/main.c",
        "#include <stdio.h>\nstruct Player {\n};\nint launch() {\n  return 0;\n}\n",
    );
    write(&repo, "vendor/ignored.c", "int ignored() { return 0; }\n");
    write(
        &repo,
        "src/generated.c",
        "// Code generated; DO NOT EDIT.\nint generated() { return 0; }\n",
    );
    write(&repo, "README.md", "class NotSource\n");

    let analysis = analyze(&repo, "generic-c");
    for (kind, name) in [
        ("file", "main.c"),
        ("module", "main"),
        ("type", "Player"),
        ("function", "launch"),
        ("import", "stdio.h"),
    ] {
        assert!(
            has_node(&analysis.records, kind, name),
            "missing {kind} {name}"
        );
    }
    for name in ["ignored", "generated", "NotSource"] {
        assert!(
            !analysis.records.iter().any(|record| match record {
                FactRecord::Node(node) => node.name == name,
                _ => false,
            }),
            "excluded source emitted {name}"
        );
    }
    assert!(analysis.records.iter().any(|record| match record {
        FactRecord::Unresolved(value) =>
            value.relation == "imports"
                && value.expression == "stdio.h"
                && value.reason == "external-target",
        _ => false,
    }));
}

type CalibrationCase<'a> = (&'a str, &'a str, &'a str, &'a [(&'a str, &'a str)]);

#[test]
fn generic_adapter_calibration_corpus_matches_go_oracle() {
    let cases: &[CalibrationCase<'_>] = &[
        (
            "generic-c",
            "main.c",
            "/*\nstruct Phantom {};\nint ghost() {}\n*/\n#include <stdio.h>\nstruct Player {};\nint launch() { return 0; }\nconst char *example = \"class Specter\";\n",
            &[
                ("import", "stdio.h"),
                ("type", "Player"),
                ("function", "launch"),
            ],
        ),
        (
            "generic-cpp",
            "fleet.cpp",
            "#include <vector>\nclass Fleet {};\nint Fleet::launch(int x) { return x; }\n",
            &[
                ("import", "vector"),
                ("type", "Fleet"),
                ("function", "launch"),
            ],
        ),
        (
            "generic-java",
            "Clock.java",
            "import java.time.Instant;\ninterface Clock {\n  default long now() { return 0; }\n}\n",
            &[
                ("import", "java.time.Instant"),
                ("interface", "Clock"),
                ("function", "now"),
            ],
        ),
        (
            "generic-kt",
            "Worker.kt",
            "import kotlin.time.Duration\ndata class Worker(val id: Int) {\n  fun run() {}\n}\n",
            &[
                ("import", "kotlin.time.Duration"),
                ("type", "Worker"),
                ("function", "run"),
            ],
        ),
        (
            "generic-swift",
            "Runner.swift",
            "import Foundation\npublic struct Runner {\n  public func execute() {}\n}\n",
            &[
                ("import", "Foundation"),
                ("type", "Runner"),
                ("function", "execute"),
            ],
        ),
        (
            "generic-php",
            "Controller.php",
            "<?php\nuse App\\Core;\nnamespace App;\nfinal class Controller {\n  public function handle() {}\n}\n",
            &[
                ("import", "App\\Core"),
                ("namespace", "App"),
                ("type", "Controller"),
                ("function", "handle"),
            ],
        ),
        (
            "generic-cs",
            "Service.cs",
            "using System.Threading.Tasks;\npublic sealed record Service {\n  public async Task RunAsync() => await Task.Yield();\n}\n",
            &[
                ("import", "System.Threading.Tasks"),
                ("type", "Service"),
                ("function", "RunAsync"),
            ],
        ),
        (
            "generic-lua",
            "service.lua",
            "local dep = require(\"dep\")\nfunction Service.run() end\n",
            &[("import", "dep"), ("function", "run")],
        ),
        (
            "generic-sh",
            "deploy.sh",
            "function deploy {\n  echo ready\n}\n",
            &[("function", "deploy")],
        ),
        (
            "generic-sql",
            "schema.sql",
            "CREATE VIEW active_users AS SELECT 1;\nCREATE TRIGGER refresh_cache AFTER INSERT ON users BEGIN SELECT 1; END;\n",
            &[("type", "active_users"), ("function", "refresh_cache")],
        ),
        (
            "generic-ps1",
            "build.ps1",
            "Import-Module \"Build.Tools\"\nfunction Start-Build { }\n",
            &[("import", "Build.Tools"), ("function", "Start-Build")],
        ),
        (
            "generic-m",
            "Clock.m",
            "#import \"Clock.h\"\n@protocol Clock\n- (long)now;\n@end\n",
            &[
                ("import", "Clock.h"),
                ("interface", "Clock"),
                ("function", "now"),
            ],
        ),
        (
            "generic-proto",
            "api.proto",
            "import weak \"types.proto\";\nenum State { UNKNOWN = 0; }\nservice Jobs {\n  rpc Start(State) returns (State);\n}\n",
            &[
                ("import", "types.proto"),
                ("type", "State"),
                ("interface", "Jobs"),
                ("function", "Start"),
            ],
        ),
        (
            "generic-sol",
            "Math.sol",
            "import \"./Base.sol\";\nlibrary Math {\n  function add() internal {}\n}\n",
            &[
                ("import", "./Base.sol"),
                ("type", "Math"),
                ("function", "add"),
            ],
        ),
        (
            "generic-pas",
            "worker.pas",
            "type TWorker = class\nend;\nprocedure Run();\nbegin\nend;\n",
            &[("type", "TWorker"), ("function", "Run")],
        ),
        (
            "generic-vb",
            "Service.vb",
            "Public Class Service\n  Public Sub Run()\n  End Sub\nEnd Class\n",
            &[("type", "Service"), ("function", "Run")],
        ),
        (
            "generic-pl",
            "service.pl",
            "sub run { return 1; }\n",
            &[("function", "run")],
        ),
        (
            "generic-r",
            "service.r",
            "run <- function(x) { x }\n",
            &[("function", "run")],
        ),
    ];

    for (index, (language, file, content, expected)) in cases.iter().enumerate() {
        let fixture = TestDirectory::new(&format!("generic-calibration-{index}"));
        let repo = fixture.path.join("repository");
        write(&repo, file, content);
        let analysis = analyze(&repo, language);
        assert_eq!(
            semantic_nodes(&analysis.records),
            expected
                .iter()
                .map(|(kind, name)| ((*kind).to_owned(), (*name).to_owned()))
                .collect::<BTreeSet<_>>(),
            "{language}"
        );
    }
}

#[test]
fn generic_adapter_recognizes_allman_functions() {
    let fixture = TestDirectory::new("generic-allman");
    let repo = fixture.path.join("repository");
    write(&repo, "main.c", "int launch()\n{\n  return 0;\n}\n");
    let analysis = analyze(&repo, "generic-c");
    assert!(has_node(&analysis.records, "function", "launch"));
}

#[test]
fn generic_adapter_incremental_ownership_matches_oracle() {
    let fixture = TestDirectory::new("generic-incremental");
    let repo = fixture.path.join("repository");
    write(&repo, "a.lua", "function alpha()\nend\n");
    write(&repo, "b.lua", "function beta()\nend\n");

    let analysis = GenericAdapter
        .analyze(&AdapterRequest {
            language: "generic-lua".into(),
            mode: AdapterMode::Incremental,
            repository: repo,
            changed_files: vec!["a.lua".into()],
            removed_files: vec!["removed.lua".into()],
            ..Default::default()
        })
        .unwrap();

    assert_eq!(analysis.header.mode.as_deref(), Some("incremental"));
    assert_eq!(
        analysis.header.changed_files.as_deref(),
        Some(&["a.lua".to_owned()][..])
    );
    assert_eq!(
        analysis.header.removed_files.as_deref(),
        Some(&["removed.lua".to_owned()][..])
    );
    assert_eq!(analysis.header.shared_complete, Some(false));
    assert!(has_node(&analysis.records, "function", "alpha"));
    assert!(!has_node(&analysis.records, "function", "beta"));
    assert!(analysis.records.iter().all(|record| match record {
        FactRecord::Node(value) => value.owner.as_deref() == Some("a.lua"),
        FactRecord::Edge(value) => value.owner.as_deref() == Some("a.lua"),
        FactRecord::Unresolved(value) => value.owner.as_deref() == Some("a.lua"),
    }));
}

#[test]
fn generic_adapter_is_deterministic_and_host_routes_fallback_languages() {
    let fixture = TestDirectory::new("generic-deterministic");
    let repo = fixture.path.join("repository");
    write(
        &repo,
        "example.java",
        "import java.util.List;\nclass Example {\n  int run() {\n  }\n}\n",
    );

    let first = analyze(&repo, "generic-java");
    let second = analyze(&repo, "generic-java");
    assert_eq!(first, second);

    let host = AdapterHost::new(fixture.path.join("adapters"));
    assert!(host.has_adapter("generic-lua"));
    assert!(!host.has_adapter("generic-md"));
    assert!(host.fingerprint("generic-lua").is_ok());
}

#[test]
fn generic_adapter_rejects_invalid_language() {
    let fixture = TestDirectory::new("generic-invalid");
    let repo = fixture.path.join("repository");
    fs::create_dir_all(&repo).unwrap();

    let error = GenericAdapter
        .analyze(&AdapterRequest {
            language: "generic-md".into(),
            repository: repo,
            ..Default::default()
        })
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unsupported generic source extension")
    );
}

fn analyze(repo: &Path, language: &str) -> lexicon::Analysis {
    GenericAdapter
        .analyze(&AdapterRequest {
            language: language.into(),
            repository: repo.to_path_buf(),
            ..Default::default()
        })
        .unwrap()
}

fn semantic_nodes(records: &[FactRecord]) -> BTreeSet<(String, String)> {
    records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) if !matches!(node.kind.as_str(), "file" | "module") => {
                Some((node.kind.clone(), node.name.clone()))
            }
            _ => None,
        })
        .collect()
}

fn has_node(records: &[FactRecord], kind: &str, name: &str) -> bool {
    records.iter().any(|record| match record {
        FactRecord::Node(node) => node.kind == kind && node.name == name,
        _ => false,
    })
}

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
