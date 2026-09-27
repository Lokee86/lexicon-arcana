use super::{RepositoryFacts, SourceSpan, UnresolvedReferenceFact};

const HEADER_V4: &str = "version\t4";

/// Encodes repository facts as canonical tab-separated UTF-8 lines.
pub fn encode_facts(facts: &RepositoryFacts) -> String {
    let mut nodes = facts.nodes.iter().collect::<Vec<_>>();
    let mut edges = facts.edges.iter().collect::<Vec<_>>();
    let mut unresolved = facts.unresolved.iter().collect::<Vec<_>>();
    nodes.sort_unstable();
    edges.sort_unstable();
    unresolved.sort_unstable();

    let mut output = String::from(HEADER_V4);
    output.push('\n');
    for node in nodes {
        output.push_str("N\t");
        push_field(&mut output, &format_id(node.key.0));
        output.push('\t');
        push_optional_field(&mut output, node.external_identity.as_deref());
        output.push('\t');
        push_field(&mut output, node.kind.as_str());
        output.push('\t');
        push_field(&mut output, &node.path);
        output.push('\t');
        push_field(&mut output, &node.name);
        output.push('\t');
        push_field(&mut output, &node.qualified_name);
        output.push('\t');
        let content_id = node
            .content_id
            .map_or_else(|| "-".to_owned(), |id| format_id(id.0));
        push_field(&mut output, &content_id);
        push_span(&mut output, node.span.as_ref());
        output.push('\n');
    }
    for edge in edges {
        output.push_str("E\t");
        push_field(&mut output, &format_id(edge.source.0));
        output.push('\t');
        push_field(&mut output, &format_id(edge.target.0));
        output.push('\t');
        push_field(&mut output, edge.relation.as_str());
        push_span(&mut output, edge.span.as_ref());
        output.push('\n');
    }
    for reference in unresolved {
        push_unresolved(&mut output, reference);
    }
    output
}

/// Encodes only unresolved references in the canonical fact-file format.
pub fn encode_unresolved_facts(unresolved: &[UnresolvedReferenceFact]) -> String {
    let mut unresolved = unresolved.iter().collect::<Vec<_>>();
    unresolved.sort_unstable();
    let mut output = String::from(HEADER_V4);
    output.push('\n');
    for reference in unresolved {
        push_unresolved(&mut output, reference);
    }
    output
}

fn push_unresolved(output: &mut String, reference: &UnresolvedReferenceFact) {
    output.push_str("U\t");
    push_field(output, &format_id(reference.source.0));
    output.push('\t');
    push_field(output, reference.relation.as_str());
    output.push('\t');
    push_field(output, reference.reason.as_str());
    output.push('\t');
    push_field(output, &reference.expression);
    output.push('\t');
    push_optional_field(output, reference.candidate_namespace.as_deref());
    output.push('\t');
    push_optional_field(output, reference.candidate_name.as_deref());
    push_span(output, reference.span.as_ref());
    output.push('\n');
}

fn push_span(output: &mut String, span: Option<&SourceSpan>) {
    if let Some(span) = span {
        for field in [
            span.path.as_str(),
            &span.start_line.to_string(),
            &span.start_column.to_string(),
            &span.end_line.to_string(),
            &span.end_column.to_string(),
        ] {
            output.push('\t');
            push_field(output, field);
        }
    } else {
        output.push_str("\t-\t-\t-\t-\t-");
    }
}

fn push_optional_field(output: &mut String, value: Option<&str>) {
    push_field(output, value.unwrap_or("-"));
}

fn push_field(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '\t' => output.push_str("\\t"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\0' => output.push_str("\\0"),
            character => output.push(character),
        }
    }
}

fn format_id(value: u64) -> String {
    format!("{value:016x}")
}
