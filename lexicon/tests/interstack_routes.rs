mod support;

use lexicon::interstack::{Library, resolve};

use support::TestDirectory;
use support::interstack::{
    assert_edge, callable_node, file_node, node_id, qualified_callable, test_id, write_fixture,
};

#[test]
fn preserves_nested_rails_namespaces_across_blank_lines() {
    let root = TestDirectory::new("interstack-rails");
    write_fixture(
        &root.path,
        "services/api-server/config/routes.rb",
        r#"Rails.application.routes.draw do
  namespace :api do
    namespace :auth do
      get "me", to: "me#show"
    end

    namespace :internal do
      namespace :player_data, path: "player-data" do
        post "stats", to: "stats#create"
      end
    end
  end

  namespace :internal do
    namespace :player_data, path: "player-data" do
      post "match-results", to: "match_results#create"
    end
  end
end
"#,
    );
    write_fixture(
        &root.path,
        "services/player-data/playerdata/rails_store.go",
        "package playerdata\nfunc loadStats() {\n\trequest, _ := s.newJSONRequest(http.MethodPost, \"/api/internal/player-data/stats\", nil)\n}\nfunc recordMatchResult() {\n\trequest, _ := s.newJSONRequest(http.MethodPost, \"/internal/player-data/match-results\", nil)\n}\n",
    );

    let result = resolve(
        &root.path,
        &[
            Library {
                language: "ruby".into(),
                repository: "space-rocks".into(),
                nodes: vec![
                    file_node('a', "services/api-server/config/routes.rb"),
                    qualified_callable(
                        'b',
                        "method",
                        "create",
                        "Api::Internal::PlayerData::StatsController#create",
                        "services/api-server/app/controllers/api/internal/player_data/stats_controller.rb",
                        1,
                    ),
                    qualified_callable(
                        'c',
                        "method",
                        "create",
                        "Internal::PlayerData::MatchResultsController#create",
                        "services/api-server/app/controllers/internal/player_data/match_results_controller.rb",
                        1,
                    ),
                ],
            },
            Library {
                language: "go".into(),
                repository: "space-rocks".into(),
                nodes: vec![
                    file_node('d', "services/player-data/playerdata/rails_store.go"),
                    callable_node(
                        'e',
                        "function",
                        "loadStats",
                        "services/player-data/playerdata/rails_store.go",
                        2,
                    ),
                    callable_node(
                        'f',
                        "function",
                        "recordMatchResult",
                        "services/player-data/playerdata/rails_store.go",
                        5,
                    ),
                ],
            },
        ],
    )
    .unwrap();

    assert_eq!(result.summary.http_contracts, 3);
    assert_eq!(result.summary.http_links, 2);
    let stats = node_id(
        &result,
        "http-endpoint",
        "POST /api/internal/player-data/stats",
    );
    let match_result = node_id(
        &result,
        "http-endpoint",
        "POST /internal/player-data/match-results",
    );
    assert_edge(&result, &test_id('e'), &stats, "calls-endpoint");
    assert_edge(&result, &stats, &test_id('b'), "handled-by");
    assert_edge(&result, &test_id('f'), &match_result, "calls-endpoint");
    assert_edge(&result, &match_result, &test_id('c'), "handled-by");
}

#[test]
fn rejects_parser_tokens_as_message_channels() {
    let root = TestDirectory::new("interstack-parser-tokens");
    write_fixture(
        &root.path,
        "arcana/src/parser.rs",
        r#"pub const PARAMETERS: &str = "parameters";
pub const TYPE_PARAMETERS: &str = "type-parameters";
pub const LANGUAGE: &str = "rust";
fn parse(token: &str) {
    match token {
        PARAMETERS => parse_parameters(),
        TYPE_PARAMETERS => parse_type_parameters(),
        LANGUAGE => parse_language(),
    }
}
"#,
    );
    let result = resolve(
        &root.path,
        &[Library {
            language: "rust".into(),
            repository: "arcana".into(),
            nodes: vec![
                file_node('a', "arcana/src/parser.rs"),
                callable_node('b', "function", "parse", "arcana/src/parser.rs", 4),
            ],
        }],
    )
    .unwrap();

    assert!(
        result
            .nodes
            .iter()
            .all(|node| node.kind != "message-channel")
    );
}
