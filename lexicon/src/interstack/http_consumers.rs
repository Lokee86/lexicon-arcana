use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use super::http::HttpContract;
use super::http_util::{http_path_shape, normalize_http_path};
use super::model::{Node, SourceFile, component_for_path};
use super::paths::last_identifier;
use super::resolver::{Resolver, line_span};

static GO_HANDLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:HandleFunc|Handle)\(\s*["']([^"']+)["']\s*,\s*([A-Za-z_][A-Za-z0-9_\.]*)"#)
        .unwrap()
});
static GO_HANDLER_BINDING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?::=|=)\s*([A-Za-z_][A-Za-z0-9_\.]*)\s*\(")
        .unwrap()
});
static GO_ROUTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\.\s*(GET|POST|PUT|PATCH|DELETE)\(\s*["']([^"']+)["']\s*,\s*([A-Za-z_][A-Za-z0-9_\.]*)"#,
    )
    .unwrap()
});

impl Resolver<'_> {
    pub(crate) fn detect_http_consumers(&mut self, file: &SourceFile) {
        if file.extension == ".rb" && file.path.ends_with("config/routes.rb") {
            self.detect_rails_routes(file);
        }
        if file.extension == ".go" {
            self.detect_go_routes(file);
        }
    }

    fn detect_go_routes(&mut self, file: &SourceFile) {
        let mut bindings = HashMap::<String, Node>::new();
        for line in &file.lines {
            let Some(capture) = GO_HANDLER_BINDING.captures(line) else {
                continue;
            };
            if let Some(provider) = self
                .index
                .callable_by_name(&last_identifier(&capture[2]), &file.path)
            {
                bindings.insert(capture[1].to_owned(), provider);
            }
        }

        for (index, line) in file.lines.iter().enumerate() {
            if let Some(capture) = GO_HANDLE.captures(line) {
                let (method, route) = split_go_route(&capture[1]);
                let handler_name = last_identifier(&capture[2]);
                let (handler_id, confidence, strategy) =
                    self.resolve_go_http_handler(&handler_name, file, index, &bindings);
                self.add_http_contract(HttpContract {
                    method,
                    path: route.clone(),
                    shape: http_path_shape(&route),
                    service: component_for_path(&file.path),
                    endpoint_id: String::new(),
                    handler_id,
                    handler_confidence: confidence,
                    handler_strategy: strategy,
                    span: Some(line_span(&file.path, index + 1, line)),
                    framework: "net/http".into(),
                    handler_name,
                });
                continue;
            }

            if let Some(capture) = GO_ROUTER.captures(line) {
                let route = normalize_http_path(&capture[2]);
                let handler_name = last_identifier(&capture[3]);
                let (handler_id, confidence, strategy) =
                    self.resolve_go_http_handler(&handler_name, file, index, &bindings);
                self.add_http_contract(HttpContract {
                    method: capture[1].to_ascii_uppercase(),
                    path: route.clone(),
                    shape: http_path_shape(&route),
                    service: component_for_path(&file.path),
                    endpoint_id: String::new(),
                    handler_id,
                    handler_confidence: confidence,
                    handler_strategy: strategy,
                    span: Some(line_span(&file.path, index + 1, line)),
                    framework: "go-router".into(),
                    handler_name,
                });
            }
        }
    }

    fn resolve_go_http_handler(
        &self,
        name: &str,
        file: &SourceFile,
        index: usize,
        bindings: &HashMap<String, Node>,
    ) -> (String, f64, String) {
        if let Some(provider) = bindings.get(name) {
            return (provider.id.clone(), 0.95, "handler-provider-binding".into());
        }
        if let Some(handler) = self.index.callable_by_name(name, &file.path) {
            return (handler.id, 1.0, "direct-handler".into());
        }
        if let Some(owner) = self.index.owner_at(&file.path, (index + 1) as u64) {
            return (owner.id, 0.7, "registration-owner".into());
        }
        (String::new(), 0.0, String::new())
    }
}

fn split_go_route(value: &str) -> (String, String) {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    if parts.len() >= 2 {
        let method = parts[0].to_ascii_uppercase();
        if matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE") {
            return (method, normalize_http_path(parts[1]));
        }
    }
    ("*".into(), normalize_http_path(value))
}
