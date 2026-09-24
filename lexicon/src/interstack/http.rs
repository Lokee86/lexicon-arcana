use std::collections::HashSet;

use serde_json::{Value, json};

use crate::{EdgeRecord, SourceSpan, UnresolvedRecord};

use super::http_util::display_http_expression;
use super::model::{Attributes, Node};
use super::paths::synthetic_path;
use super::resolver::{Resolver, attributes, span_sort_key};

#[derive(Debug, Clone)]
pub(crate) struct HttpContract {
    pub method: String,
    pub path: String,
    pub shape: String,
    pub service: String,
    pub endpoint_id: String,
    pub handler_id: String,
    pub handler_confidence: f64,
    pub handler_strategy: String,
    pub span: Option<SourceSpan>,
    pub framework: String,
    pub handler_name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct HttpProducer {
    pub source_id: String,
    pub method: String,
    pub path: String,
    pub shape: String,
    pub span: Option<SourceSpan>,
    pub evidence: String,
}

impl Resolver {
    pub(crate) fn resolve_http_producers(&mut self) {
        let mut seen = HashSet::new();
        for producer in self.http_sources.clone() {
            let key = format!(
                "{}\0{}\0{}\0{}",
                producer.source_id,
                producer.method,
                producer.shape,
                span_sort_key(producer.span.as_ref())
            );
            if !seen.insert(key) {
                continue;
            }

            let mut candidates = self
                .http
                .iter()
                .filter(|contract| {
                    contract.shape == producer.shape
                        && (producer.method.is_empty()
                            || contract.method == "*"
                            || contract.method == producer.method)
                })
                .cloned()
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| left.endpoint_id.cmp(&right.endpoint_id));

            if candidates.len() == 1 {
                let (confidence, strategy) =
                    if producer.method.is_empty() || candidates[0].method == "*" {
                        (0.85, "unique-route")
                    } else {
                        (1.0, "method-and-route")
                    };
                self.add_edge(EdgeRecord {
                    attributes: attributes([
                        ("confidence", json!(confidence)),
                        ("evidence", json!([producer.evidence])),
                        ("strategy", json!(strategy)),
                        ("transport", json!("http")),
                    ]),
                    owner: None,
                    relation: "calls-endpoint".into(),
                    source: producer.source_id,
                    span: producer.span,
                    target: candidates[0].endpoint_id.clone(),
                });
                self.result.summary.http_links += 1;
                continue;
            }

            let reason = if candidates.len() > 1 {
                "ambiguous-target"
            } else {
                "missing-target"
            };
            self.add_unresolved(UnresolvedRecord {
                attributes: attributes([
                    ("candidate_count", json!(candidates.len())),
                    ("transport", json!("http")),
                ]),
                candidate_name: None,
                candidate_namespace: None,
                expression: display_http_expression(&producer.method, &producer.path),
                owner: None,
                reason: reason.into(),
                relation: "calls-endpoint".into(),
                source: producer.source_id,
                span: producer.span,
            });
        }
    }

    pub(crate) fn add_http_contract(&mut self, mut contract: HttpContract) {
        let identity = format!(
            "{}\0{}\0{}",
            contract.service, contract.method, contract.path
        );
        contract.endpoint_id = crate::node_id(super::LANGUAGE, "http-endpoint", &identity);
        let name = display_http_expression(&contract.method, &contract.path);

        let mut node_attributes = Attributes::new();
        node_attributes.insert("framework".into(), json!(contract.framework.clone()));
        node_attributes.insert("method".into(), json!(contract.method.clone()));
        node_attributes.insert("route".into(), json!(contract.path.clone()));
        node_attributes.insert("service".into(), json!(contract.service.clone()));
        node_attributes.insert("transport".into(), json!("http"));
        self.add_node(Node {
            id: contract.endpoint_id.clone(),
            kind: "http-endpoint".into(),
            name: name.clone(),
            path: synthetic_path("http", &identity),
            qualified_name: format!("http:{}:{name}", contract.service),
            span: None,
            attributes: node_attributes,
        });
        self.http.push(contract.clone());
        self.result.summary.http_contracts += 1;

        if !contract.handler_id.is_empty() {
            let confidence = if contract.handler_confidence == 0.0 {
                1.0
            } else {
                contract.handler_confidence
            };
            let mut values = vec![
                ("confidence", json!(confidence)),
                ("framework", json!(contract.framework)),
                ("transport", json!("http")),
            ];
            if !contract.handler_strategy.is_empty() {
                values.push(("strategy", Value::String(contract.handler_strategy)));
            }
            self.add_edge(EdgeRecord {
                attributes: attributes(values),
                owner: None,
                relation: "handled-by".into(),
                source: contract.endpoint_id,
                span: contract.span,
                target: contract.handler_id,
            });
        } else {
            self.add_unresolved(UnresolvedRecord {
                attributes: attributes([
                    ("framework", json!(contract.framework)),
                    ("transport", json!("http")),
                ]),
                candidate_name: None,
                candidate_namespace: None,
                expression: contract.handler_name,
                owner: None,
                reason: "missing-target".into(),
                relation: "handled-by".into(),
                source: contract.endpoint_id,
                span: contract.span,
            });
        }
    }
}
