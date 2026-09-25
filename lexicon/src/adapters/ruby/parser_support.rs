fn base_node<'a>(_: &str, node: Node<'a>) -> Node<'a> {
    node.child_by_field_name("superclass").unwrap_or(node)
}

fn node_span(n: Node<'_>, path: &str) -> SourceSpan {
    SourceSpan {
        path: path.into(),
        start_line: n.start_position().row as u64 + 1,
        start_column: n.start_position().column as u64 + 1,
        end_line: n.end_position().row as u64 + 1,
        end_column: n.end_position().column as u64 + 1,
    }
}
fn normalized(v: &[String]) -> Vec<String> {
    let mut x = v.iter().map(|p| p.replace('\\', "/")).collect::<Vec<_>>();
    x.sort();
    x.dedup();
    x
}
trait ModeExt {
    fn then_some<T>(self, v: T) -> Option<T>;
}
impl ModeExt for AdapterMode {
    fn then_some<T>(self, v: T) -> Option<T> {
        if self == AdapterMode::Incremental {
            Some(v)
        } else {
            None
        }
    }
}
