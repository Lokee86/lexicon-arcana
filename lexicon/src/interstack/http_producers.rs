use serde_json::json;

use crate::{SourceSpan, UnresolvedRecord};

use super::http::HttpProducer;
use super::http_util::{
    HTTP_HELPER_CALL, QUOTED_HTTP_VALUE, compact_evidence, http_call_block, http_method_from_line,
    http_path_shape, is_http_provider_name, looks_like_http_client_call, normalize_http_path,
    unique_http_provider,
};
use super::model::SourceFile;
use super::resolver::{Resolver, attributes, line_span};

impl Resolver {
    pub(crate) fn collect_http_path_providers(&mut self, file: &SourceFile) {
        if file.extension == ".rb" && file.path.ends_with("config/routes.rb") {
            return;
        }
        for (index, line) in file.lines.iter().enumerate() {
            let Some(owner) = self.index.owner_at(&file.path, (index + 1) as u64) else {
                continue;
            };
            if !is_http_provider_name(&owner.name) || !line.to_ascii_lowercase().contains("return")
            {
                continue;
            }
            for capture in QUOTED_HTTP_VALUE.captures_iter(line) {
                let normalized = normalize_http_path(&capture[1]);
                if normalized.is_empty() {
                    continue;
                }
                let producer = HttpProducer {
                    source_id: owner.id.clone(),
                    method: String::new(),
                    path: normalized.clone(),
                    shape: http_path_shape(&normalized),
                    span: Some(line_span(&file.path, index + 1, line)),
                    evidence: line.trim().to_owned(),
                };
                self.http_providers
                    .entry(owner.name.clone())
                    .or_default()
                    .push(producer.clone());
                self.http_sources.push(producer);
            }
        }
    }

    pub(crate) fn detect_http_producers(&mut self, file: &SourceFile) {
        if file.extension == ".rb" && file.path.ends_with("config/routes.rb") {
            return;
        }
        for (index, line) in file.lines.iter().enumerate() {
            if !looks_like_http_client_call(line) {
                continue;
            }
            let Some(owner) = self.index.owner_at(&file.path, (index + 1) as u64) else {
                continue;
            };
            let (block, end) = http_call_block(&file.lines, index);
            let method = http_method_from_line(&block);
            let span = block_span(&file.path, index + 1, end + 1, &block);
            let evidence = compact_evidence(&block);

            for capture in QUOTED_HTTP_VALUE.captures_iter(&block) {
                self.append_http_producer(&owner.id, &method, &capture[1], &span, &evidence);
            }
            for capture in HTTP_HELPER_CALL.captures_iter(&block) {
                let helper_name = &capture[1];
                if !is_http_provider_name(helper_name) {
                    continue;
                }
                let providers = self
                    .http_providers
                    .get(helper_name)
                    .cloned()
                    .unwrap_or_default();
                if let Some(provider) = unique_http_provider(&providers) {
                    self.http_sources.push(HttpProducer {
                        source_id: owner.id.clone(),
                        method: method.clone(),
                        path: provider.path,
                        shape: provider.shape,
                        span: Some(span.clone()),
                        evidence: evidence.clone(),
                    });
                    continue;
                }

                let reason = if providers.len() > 1 {
                    "ambiguous-target"
                } else {
                    "missing-target"
                };
                self.add_unresolved(UnresolvedRecord {
                    attributes: attributes([
                        ("candidate_count", json!(providers.len())),
                        ("strategy", json!("route-provider")),
                        ("transport", json!("http")),
                    ]),
                    candidate_name: None,
                    candidate_namespace: None,
                    expression: format!("{helper_name}()"),
                    owner: None,
                    reason: reason.into(),
                    relation: "calls-endpoint".into(),
                    source: owner.id.clone(),
                    span: Some(span.clone()),
                });
            }
        }
    }

    fn append_http_producer(
        &mut self,
        source_id: &str,
        method: &str,
        raw_path: &str,
        span: &SourceSpan,
        evidence: &str,
    ) {
        let normalized = normalize_http_path(raw_path);
        if normalized.is_empty() {
            return;
        }
        self.http_sources.push(HttpProducer {
            source_id: source_id.to_owned(),
            method: method.to_owned(),
            path: normalized.clone(),
            shape: http_path_shape(&normalized),
            span: Some(span.clone()),
            evidence: evidence.to_owned(),
        });
    }
}

fn block_span(path: &str, start_line: usize, end_line: usize, text: &str) -> SourceSpan {
    let end_column = text
        .lines()
        .last()
        .map(|line| line.chars().count() + 1)
        .unwrap_or(2)
        .max(2);
    SourceSpan {
        path: path.to_owned(),
        start_line: start_line as u64,
        start_column: 1,
        end_line: end_line as u64,
        end_column: end_column as u64,
    }
}
