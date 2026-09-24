mod support;

use lexicon::interstack::{Library, resolve};

use support::TestDirectory;
use support::interstack::{assert_edge, callable_node, file_node, node_id, test_id, write_fixture};

#[test]
fn links_process_cli_protocol_and_state_boundaries() {
    let root = TestDirectory::new("interstack-boundaries");
    write_fixture(
        &root.path,
        "internal/arcanagraph/protocol.go",
        "package arcanagraph\nimport \"os/exec\"\nfunc runProtocol(ctx Context, command string) {\n\texec.CommandContext(ctx, command, \"protocol\", \"--snapshot\", snapshot)\n}\n",
    );
    write_fixture(
        &root.path,
        "internal/lexiconfacts/state.go",
        "package lexiconfacts\nimport \"os/exec\"\nfunc ResolveExport(ctx Context, command string) {\n\targs := []string{\"export\", \"--snapshot\", snapshot}\n\texec.CommandContext(ctx, command, args...)\n}\n",
    );
    write_fixture(
        &root.path,
        "lexicon/internal/cli/cli.go",
        "package cli\nfunc Run(arguments []string) {\n\tswitch arguments[0] {\n\tcase \"scan\":\n\t\trunScan()\n\tcase \"export\":\n\t\trunExport()\n\t}\n}\n",
    );
    write_fixture(
        &root.path,
        "lexicon/internal/objectstore/store.go",
        "package objectstore\nfunc (s Store) Publish() { writeAtomic(join(s.Root, \"CURRENT\")) }\nfunc (s Store) Current() { readFile(join(s.Root, \"CURRENT\")) }\n",
    );
    write_fixture(
        &root.path,
        "arcana/src/main.rs",
        "fn main() {\n    match parse() {\n        Ok(cli::Command::Sync(command)) => run_sync(command),\n        Ok(cli::Command::Protocol(command)) => run_protocol(command),\n    }\n}\n",
    );
    write_fixture(
        &root.path,
        "arcana/src/cli_protocol.rs",
        "pub fn run_protocol(command: &ProtocolCommand) {\n    serve_jsonl(command);\n}\n",
    );
    write_fixture(
        &root.path,
        "arcana/src/cli_sync.rs",
        "pub fn run_sync(command: &SyncCommand) { publish_current(command); }\nfn publish_current(state: &Path) { replace_file(state.join(\"CURRENT\")); }\nfn read_current(state: &Path) { read_to_string(state.join(\"CURRENT\")); }\n",
    );
    write_fixture(
        &root.path,
        "arcana/src/lexicon/snapshot.rs",
        "pub fn current(root: &Path) { read(root.join(\"CURRENT\")); }\npub fn load(root: &Path) { read(root.join(\"snapshots\")); }\n",
    );

    let result = resolve(
        &root.path,
        &[
            Library {
                language: "go".into(),
                repository: "warlock".into(),
                nodes: vec![
                    file_node('a', "internal/arcanagraph/protocol.go"),
                    callable_node(
                        'b',
                        "function",
                        "runProtocol",
                        "internal/arcanagraph/protocol.go",
                        3,
                    ),
                    file_node('c', "internal/lexiconfacts/state.go"),
                    callable_node(
                        'd',
                        "function",
                        "ResolveExport",
                        "internal/lexiconfacts/state.go",
                        3,
                    ),
                    file_node('e', "lexicon/internal/cli/cli.go"),
                    callable_node('f', "function", "Run", "lexicon/internal/cli/cli.go", 2),
                    file_node('1', "lexicon/internal/objectstore/store.go"),
                    callable_node(
                        '2',
                        "method",
                        "Publish",
                        "lexicon/internal/objectstore/store.go",
                        2,
                    ),
                    callable_node(
                        '3',
                        "method",
                        "Current",
                        "lexicon/internal/objectstore/store.go",
                        3,
                    ),
                ],
            },
            Library {
                language: "rust".into(),
                repository: "warlock".into(),
                nodes: vec![
                    file_node('4', "arcana/src/main.rs"),
                    callable_node('5', "function", "main", "arcana/src/main.rs", 1),
                    file_node('6', "arcana/src/cli_protocol.rs"),
                    callable_node(
                        '7',
                        "function",
                        "run_protocol",
                        "arcana/src/cli_protocol.rs",
                        1,
                    ),
                    file_node('8', "arcana/src/cli_sync.rs"),
                    callable_node('9', "function", "run_sync", "arcana/src/cli_sync.rs", 1),
                    callable_node(
                        'g',
                        "function",
                        "publish_current",
                        "arcana/src/cli_sync.rs",
                        2,
                    ),
                    callable_node('h', "function", "read_current", "arcana/src/cli_sync.rs", 3),
                    file_node('i', "arcana/src/lexicon/snapshot.rs"),
                    callable_node(
                        'j',
                        "function",
                        "current",
                        "arcana/src/lexicon/snapshot.rs",
                        1,
                    ),
                    callable_node('k', "function", "load", "arcana/src/lexicon/snapshot.rs", 2),
                ],
            },
        ],
    )
    .unwrap();

    let arcana_process = node_id(&result, "process", "arcana");
    let lexicon_process = node_id(&result, "process", "lexicon");
    let protocol = node_id(&result, "protocol", "arcana.query.v1");
    let arcana_protocol = node_id(&result, "cli-command", "arcana protocol");
    let lexicon_export = node_id(&result, "cli-command", "lexicon export");
    let lexicon_current = node_id(&result, "state-path", ".lexicon/CURRENT");
    let arcana_current = node_id(&result, "state-path", ".arcana/CURRENT");

    assert_edge(&result, &test_id('b'), &arcana_process, "invokes-process");
    assert_edge(&result, &test_id('b'), &arcana_protocol, "calls");
    assert_edge(&result, &test_id('d'), &lexicon_process, "invokes-process");
    assert_edge(&result, &test_id('d'), &lexicon_export, "calls");
    assert_edge(&result, &test_id('b'), &protocol, "produces-message");
    assert_edge(&result, &protocol, &test_id('7'), "consumes-message");
    assert_edge(&result, &test_id('2'), &lexicon_current, "writes");
    assert_edge(&result, &test_id('d'), &lexicon_current, "reads");
    assert_edge(&result, &test_id('9'), &lexicon_current, "reads");
    assert_edge(&result, &test_id('g'), &arcana_current, "writes");
    assert_edge(&result, &test_id('h'), &arcana_current, "reads");
}

#[test]
fn links_boundary_environment_configuration() {
    let root = TestDirectory::new("interstack-boundary-config");
    write_fixture(
        &root.path,
        "internal/app/engine_specs.go",
        "package app\nvar lexiconEngine = engineSpec{commandEnv: \"GRIMOIRE_LEXICON_COMMAND\"}\nvar arcanaEngine = engineSpec{commandEnv: \"GRIMOIRE_ARCANA_COMMAND\"}\n",
    );
    write_fixture(
        &root.path,
        "lexicon/internal/config/config.go",
        "package config\nimport \"os\"\nfunc StateRoot() string { return os.Getenv(\"LEXICON_STATE_DIR\") }\n",
    );

    let result = resolve(
        &root.path,
        &[Library {
            language: "go".into(),
            repository: "warlock".into(),
            nodes: vec![
                file_node('a', "internal/app/engine_specs.go"),
                file_node('b', "lexicon/internal/config/config.go"),
                callable_node(
                    'c',
                    "function",
                    "StateRoot",
                    "lexicon/internal/config/config.go",
                    3,
                ),
            ],
        }],
    )
    .unwrap();

    let lexicon_command = node_id(&result, "config-key", "GRIMOIRE_LEXICON_COMMAND");
    let arcana_command = node_id(&result, "config-key", "GRIMOIRE_ARCANA_COMMAND");
    let state = node_id(&result, "config-key", "LEXICON_STATE_DIR");
    assert_edge(&result, &test_id('a'), &lexicon_command, "reads-config");
    assert_edge(&result, &test_id('a'), &arcana_command, "reads-config");
    assert_edge(&result, &test_id('c'), &state, "reads-config");
}
