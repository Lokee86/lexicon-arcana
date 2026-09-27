use std::sync::LazyLock;

use regex::Regex;

use super::http::HttpContract;
use super::http_util::{http_path_shape, normalize_http_path};
use super::model::{SourceFile, component_for_path};
use super::paths::camelize;
use super::resolver::{Resolver, line_span};

#[derive(Debug)]
struct RailsNamespace {
    depth: usize,
    name: String,
    path: String,
}

static RAILS_NAMESPACE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*namespace\s+:([A-Za-z_][A-Za-z0-9_]*)(?:\s*,\s*path:\s*["']([^"']+)["'])?\s+do\b"#,
    )
    .unwrap()
});
static RAILS_ROUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*(get|post|put|patch|delete)\s+["']([^"']+)["']\s*(?:(?:,?\s*to:\s*|=>\s*)["']([^"']+)#([^"']+)["'])"#,
    )
    .unwrap()
});

impl Resolver<'_> {
    pub(crate) fn detect_rails_routes(&mut self, file: &SourceFile) {
        let mut stack = Vec::<RailsNamespace>::new();
        let mut depth = 0_usize;
        for (index, line) in file.lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed == "end" || trimmed.starts_with("end #") {
                depth = depth.saturating_sub(1);
                while stack.last().is_some_and(|item| item.depth > depth) {
                    stack.pop();
                }
                continue;
            }

            if let Some(capture) = RAILS_NAMESPACE.captures(line) {
                let name = capture[1].to_owned();
                let path = capture
                    .get(2)
                    .map(|value| value.as_str().to_owned())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| name.replace('_', "-"));
                depth += 1;
                stack.push(RailsNamespace { depth, name, path });
                continue;
            }

            if let Some(capture) = RAILS_ROUTE.captures(line) {
                let mut prefix = stack
                    .iter()
                    .map(|item| item.path.clone())
                    .collect::<Vec<_>>();
                let mut controller_parts = stack
                    .iter()
                    .map(|item| camelize(&item.name))
                    .collect::<Vec<_>>();
                prefix.push(capture[2].to_owned());
                let route = normalize_http_path(&format!("/{}", prefix.join("/")));
                controller_parts
                    .extend(capture[3].trim_start_matches('/').split('/').map(camelize));
                if let Some(last) = controller_parts.last_mut() {
                    last.push_str("Controller");
                    let handler_name = format!("{}#{}", controller_parts.join("::"), &capture[4]);
                    let handler_id = self
                        .index
                        .exact_qname(&handler_name)
                        .map(|node| node.id)
                        .unwrap_or_default();
                    self.add_http_contract(HttpContract {
                        method: capture[1].to_ascii_uppercase(),
                        path: route.clone(),
                        shape: http_path_shape(&route),
                        service: component_for_path(&file.path),
                        endpoint_id: String::new(),
                        handler_id,
                        handler_confidence: 0.0,
                        handler_strategy: String::new(),
                        span: Some(line_span(&file.path, index + 1, line)),
                        framework: "rails".into(),
                        handler_name,
                    });
                }
            }

            if trimmed.ends_with(" do") {
                depth += 1;
            }
        }
    }
}
