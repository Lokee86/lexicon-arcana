use crate::EdgeRecord;

use super::model::{Node, SourceFile};
use super::paths::synthetic_path;
use super::resolver::Resolver;

impl Resolver<'_> {
    pub(crate) fn detect_arcana_protocol(&mut self, file: &SourceFile) {
        let path = file.path.replace('\\', "/").to_ascii_lowercase();
        let producer = path.ends_with("internal/arcanagraph/protocol.go");
        let consumer = path.ends_with("arcana/src/cli_protocol.rs");
        if !producer && !consumer {
            return;
        }
        let protocol = self.add_protocol("arcana.query.v1");
        let process = self.add_process("arcana");
        let command = self.add_cli_command("arcana", "protocol");
        if producer {
            if let Some(owner) = self.index.callable_by_name("runProtocol", &file.path) {
                self.add_edge(edge(&owner.id, &protocol.id, "produces-message"));
                self.add_edge(edge(&owner.id, &command.id, "calls"));
                self.result.summary.protocol_links += 1;
            }
        } else if let Some(owner) = self.index.callable_by_name("run_protocol", &file.path) {
            self.add_edge(edge(&protocol.id, &owner.id, "consumes-message"));
            self.add_edge(edge(&owner.id, &command.id, "defines"));
            self.result.summary.protocol_links += 1;
        }
        self.add_edge(edge(&process.id, &command.id, "contains"));
        self.add_edge(edge(&command.id, &protocol.id, "contains"));
    }

    fn add_protocol(&mut self, name: &str) -> Node {
        let identity = format!("protocol\0{name}");
        let id = crate::node_id(super::LANGUAGE, "protocol", &identity);
        if let Some(node) = self.nodes.get(&id) {
            return node.clone();
        }
        let node = Node {
            id,
            kind: "protocol".into(),
            name: name.to_owned(),
            path: synthetic_path("protocols", &identity),
            qualified_name: format!("protocol:{name}"),
            span: None,
            attributes: Default::default(),
        };
        self.add_node(node.clone());
        self.result.summary.protocols += 1;
        node
    }
}

fn edge(source: &str, target: &str, relation: &str) -> EdgeRecord {
    EdgeRecord {
        attributes: None,
        owner: None,
        relation: relation.to_owned(),
        source: source.to_owned(),
        span: None,
        target: target.to_owned(),
    }
}
