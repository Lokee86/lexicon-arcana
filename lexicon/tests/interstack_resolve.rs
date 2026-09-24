mod support;

use lexicon::interstack::{Library, resolve};

use support::TestDirectory;
use support::interstack::{
    assert_edge, callable_node, file_node, node_id, qualified_callable, test_id, write_fixture,
};

#[test]
fn links_http_packets_and_config_across_languages() {
    let root = TestDirectory::new("interstack-cross-language");
    write_fixture(
        &root.path,
        "client/api.gd",
        r#"func auth_me_path():
	return "%s/api/auth/me" % API_BASE

func create_room_packet():
	var packet := {}
	packet[FIELD_TYPE] = "create_room_request"
	return packet

const API_URL_ENV := "API_URL"

func api_url():
	return OS.get_environment(API_URL_ENV)
"#,
    );
    write_fixture(
        &root.path,
        "client/auth_api_client.gd",
        r#"func get_current_user():
	return await api_http_client.get_json(ApiConfigScript.auth_me_path(), token, trace_id)
"#,
    );
    write_fixture(
        &root.path,
        "services/api-server/config/routes.rb",
        r#"Rails.application.routes.draw do
  namespace :api do
    namespace :auth do
      get "me", to: "me#show"
    end
  end
end
"#,
    );
    write_fixture(
        &root.path,
        "services/api-server/app/controllers/api/auth/me_controller.rb",
        "module Api\n  module Auth\n    class MeController\n      def show\n      end\n    end\n  end\nend\n",
    );
    write_fixture(
        &root.path,
        "services/game-server/internal/game/packets.go",
        "package game\nconst PacketTypeCreateRoomRequest = \"create_room_request\"\n",
    );
    write_fixture(
        &root.path,
        "services/game-server/internal/networking/inbound/lobby.go",
        "package inbound\n\nfunc HandleLobby(packet Packet) {\n\tswitch packet.Type {\n\tcase game.PacketTypeCreateRoomRequest:\n\t\tcreateRoom()\n\t}\n}\n",
    );

    let libraries = vec![
        Library {
            language: "gdscript".into(),
            repository: "space-rocks".into(),
            nodes: vec![
                file_node('a', "client/api.gd"),
                callable_node('b', "method", "auth_me_path", "client/api.gd", 1),
                callable_node('c', "method", "create_room_packet", "client/api.gd", 4),
                callable_node('d', "method", "api_url", "client/api.gd", 11),
                file_node('4', "client/auth_api_client.gd"),
                callable_node(
                    '5',
                    "method",
                    "get_current_user",
                    "client/auth_api_client.gd",
                    1,
                ),
            ],
        },
        Library {
            language: "ruby".into(),
            repository: "space-rocks".into(),
            nodes: vec![
                file_node('e', "services/api-server/config/routes.rb"),
                file_node(
                    'f',
                    "services/api-server/app/controllers/api/auth/me_controller.rb",
                ),
                qualified_callable(
                    '1',
                    "method",
                    "show",
                    "Api::Auth::MeController#show",
                    "services/api-server/app/controllers/api/auth/me_controller.rb",
                    4,
                ),
            ],
        },
        Library {
            language: "go".into(),
            repository: "space-rocks".into(),
            nodes: vec![
                file_node(
                    '2',
                    "services/game-server/internal/networking/inbound/lobby.go",
                ),
                callable_node(
                    '3',
                    "function",
                    "HandleLobby",
                    "services/game-server/internal/networking/inbound/lobby.go",
                    3,
                ),
            ],
        },
    ];

    let result = resolve(&root.path, &libraries).unwrap();
    assert_eq!(result.summary.http_contracts, 1);
    assert_eq!(result.summary.http_links, 2);
    assert_eq!(result.summary.message_channels, 1);
    assert_eq!(result.summary.message_links, 2);
    assert_eq!(result.summary.config_keys, 1);

    let http = node_id(&result, "http-endpoint", "GET /api/auth/me");
    let message = node_id(&result, "message-channel", "create_room_request");
    let config = node_id(&result, "config-key", "API_URL");
    assert_edge(&result, &test_id('b'), &http, "calls-endpoint");
    assert_edge(&result, &test_id('5'), &http, "calls-endpoint");
    assert_edge(&result, &http, &test_id('1'), "handled-by");
    assert_edge(&result, &test_id('c'), &message, "publishes");
    assert_edge(&result, &message, &test_id('3'), "consumes");
    assert_edge(&result, &test_id('d'), &config, "reads-config");
}

#[test]
fn links_go_serve_mux_through_bound_handler_provider() {
    let root = TestDirectory::new("interstack-go-binding");
    write_fixture(
        &root.path,
        "client/api_config.gd",
        "func player_data_profile_path():\n\treturn \"%s/api/player-data/profile\" % player_data_base_url()\n",
    );
    write_fixture(
        &root.path,
        "client/profile_api_client.gd",
        "func load_profile():\n\treturn await api_http_client.post_json(ApiConfigScript.player_data_profile_path(), {})\n",
    );
    write_fixture(
        &root.path,
        "services/game-server/cmd/game-server/main.go",
        "package main\nfunc runWithContext() {\n\tplayerDataProfileHandler := newPlayerDataProfileHTTPHandler()\n\tmux.Handle(\"POST /api/player-data/profile\", playerDataProfileHandler)\n}\nfunc newPlayerDataProfileHTTPHandler() http.Handler {\n\treturn nil\n}\n",
    );

    let result = resolve(
        &root.path,
        &[
            Library {
                language: "gdscript".into(),
                repository: "space-rocks".into(),
                nodes: vec![
                    file_node('a', "client/api_config.gd"),
                    callable_node(
                        'b',
                        "method",
                        "player_data_profile_path",
                        "client/api_config.gd",
                        1,
                    ),
                    file_node('c', "client/profile_api_client.gd"),
                    callable_node(
                        'd',
                        "method",
                        "load_profile",
                        "client/profile_api_client.gd",
                        1,
                    ),
                ],
            },
            Library {
                language: "go".into(),
                repository: "space-rocks".into(),
                nodes: vec![
                    file_node('e', "services/game-server/cmd/game-server/main.go"),
                    callable_node(
                        'f',
                        "function",
                        "runWithContext",
                        "services/game-server/cmd/game-server/main.go",
                        2,
                    ),
                    callable_node(
                        '1',
                        "function",
                        "newPlayerDataProfileHTTPHandler",
                        "services/game-server/cmd/game-server/main.go",
                        6,
                    ),
                ],
            },
        ],
    )
    .unwrap();

    assert_eq!(result.summary.http_contracts, 1);
    assert_eq!(result.summary.http_links, 2);
    let endpoint = node_id(&result, "http-endpoint", "POST /api/player-data/profile");
    assert_edge(&result, &test_id('b'), &endpoint, "calls-endpoint");
    assert_edge(&result, &test_id('d'), &endpoint, "calls-endpoint");
    assert_edge(&result, &endpoint, &test_id('1'), "handled-by");
}
