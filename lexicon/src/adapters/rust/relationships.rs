use crate::adapters::rust::model::Context;
use crate::adapters::rust::paths::span_value;
use proc_macro2::Span;

pub(crate) fn finalize(context: &mut Context) {
    crate::adapters::rust::imports::resolve_all(context);
    crate::adapters::rust::implementations::finalize(context);
    crate::adapters::rust::semantic::analyze(context);
}

pub(crate) fn define_and_contain(
    context: &mut Context,
    owner: &str,
    target: &str,
    span: Span,
    path: &str,
) {
    let span = span_value(span, path);
    context
        .facts
        .add_edge(owner, target, "contains", span.clone());
    context.facts.add_edge(owner, target, "defines", span);
}
