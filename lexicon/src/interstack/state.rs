use serde_json::json;

use crate::EdgeRecord;

use super::model::{Attributes, Node, SourceFile, normalize_source_path};
use super::paths::synthetic_path;
use super::resolver::{Resolver, attributes};

struct StateRole {
    callable: &'static str,
    path: &'static str,
    relation: &'static str,
}

impl Resolver<'_> {
    pub(crate) fn detect_state_contracts(&mut self, file: &SourceFile) {
        let path = normalize_source_path(&file.path).to_ascii_lowercase();
        let roles: &[StateRole] = if path.ends_with("lexicon/internal/objectstore/store.go") {
            &[
                StateRole {
                    callable: "Publish",
                    path: ".lexicon",
                    relation: "writes",
                },
                StateRole {
                    callable: "Publish",
                    path: ".lexicon/CURRENT",
                    relation: "writes",
                },
                StateRole {
                    callable: "Publish",
                    path: ".lexicon/snapshots",
                    relation: "writes",
                },
                StateRole {
                    callable: "Current",
                    path: ".lexicon/CURRENT",
                    relation: "reads",
                },
                StateRole {
                    callable: "Current",
                    path: ".lexicon/snapshots",
                    relation: "reads",
                },
            ]
        } else if path.ends_with("internal/lexiconfacts/state.go") {
            &[
                StateRole {
                    callable: "ResolveExport",
                    path: ".lexicon",
                    relation: "reads",
                },
                StateRole {
                    callable: "ResolveExport",
                    path: ".lexicon/CURRENT",
                    relation: "reads",
                },
                StateRole {
                    callable: "ResolveExport",
                    path: ".lexicon/snapshots",
                    relation: "reads",
                },
            ]
        } else if path.ends_with("internal/arcanagraph/state.go") {
            &[
                StateRole {
                    callable: "ResolveSnapshot",
                    path: ".lexicon/CURRENT",
                    relation: "reads",
                },
                StateRole {
                    callable: "ResolveSnapshot",
                    path: ".arcana",
                    relation: "reads",
                },
                StateRole {
                    callable: "ResolveSnapshot",
                    path: ".arcana/CURRENT",
                    relation: "reads",
                },
                StateRole {
                    callable: "ResolveSnapshot",
                    path: ".arcana/snapshots",
                    relation: "reads",
                },
            ]
        } else if path.ends_with("arcana/src/lexicon/snapshot.rs") {
            &[
                StateRole {
                    callable: "current",
                    path: ".lexicon/CURRENT",
                    relation: "reads",
                },
                StateRole {
                    callable: "load",
                    path: ".lexicon/snapshots",
                    relation: "reads",
                },
            ]
        } else if path.ends_with("arcana/src/cli_sync.rs") {
            &[
                StateRole {
                    callable: "run_sync",
                    path: ".lexicon",
                    relation: "reads",
                },
                StateRole {
                    callable: "run_sync",
                    path: ".lexicon/CURRENT",
                    relation: "reads",
                },
                StateRole {
                    callable: "run_sync",
                    path: ".lexicon/snapshots",
                    relation: "reads",
                },
                StateRole {
                    callable: "run_sync",
                    path: ".arcana",
                    relation: "writes",
                },
                StateRole {
                    callable: "run_sync",
                    path: ".arcana/snapshots",
                    relation: "writes",
                },
                StateRole {
                    callable: "publish_current",
                    path: ".arcana/CURRENT",
                    relation: "writes",
                },
                StateRole {
                    callable: "read_current",
                    path: ".arcana/CURRENT",
                    relation: "reads",
                },
            ]
        } else {
            return;
        };

        for role in roles {
            let Some(owner) = self.index.callable_by_name(role.callable, &file.path) else {
                continue;
            };
            let node = self.add_state_path(role.path);
            self.add_edge(EdgeRecord {
                attributes: attributes([
                    ("confidence", json!(1.0)),
                    ("transport", json!("filesystem-state")),
                ]),
                owner: None,
                relation: role.relation.into(),
                source: owner.id,
                span: None,
                target: node.id,
            });
            self.result.summary.state_links += 1;
        }
    }

    fn add_state_path(&mut self, path: &str) -> Node {
        let identity = format!("state-path\0{path}");
        let id = crate::node_id(super::LANGUAGE, "state-path", &identity);
        if let Some(node) = self.nodes.get(&id) {
            return node.clone();
        }
        let mut node_attributes = Attributes::new();
        node_attributes.insert("path".into(), json!(path));
        node_attributes.insert("transport".into(), json!("filesystem-state"));
        let node = Node {
            id,
            kind: "state-path".into(),
            name: path.to_owned(),
            path: synthetic_path("state", &identity),
            qualified_name: format!("state:{path}"),
            span: None,
            attributes: node_attributes,
        };
        self.add_node(node.clone());
        self.result.summary.state_paths += 1;
        node
    }
}
