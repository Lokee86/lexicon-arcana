pub(super) mod bindings;
mod calls;
mod relationships;
mod shapes;

use super::facts::Facts;
use bindings::BindingResolver;

pub fn resolve(facts: &mut Facts) {
    let started = crate::perf::start();
    bindings::resolve_imports(facts);
    if let Some(started) = started {
        crate::perf::emit(
            "python.import_resolution",
            started.elapsed(),
            &[
                ("imports", facts.imports.len() as u64),
                ("scope_bindings", facts.scope_bindings.len() as u64),
                ("module_bindings", facts.module_bindings.len() as u64),
            ],
        );
    }

    let mut bindings = BindingResolver::new(facts);

    let started = crate::perf::start();
    relationships::resolve_inheritance(facts, &mut bindings);
    if let Some(started) = started {
        crate::perf::emit(
            "python.inheritance_resolution",
            started.elapsed(),
            &[("classes", facts.classes.len() as u64)],
        );
    }

    let started = crate::perf::start();
    relationships::emit_overrides(facts, &mut bindings);
    if let Some(started) = started {
        crate::perf::emit(
            "python.override_resolution",
            started.elapsed(),
            &[("functions", facts.functions.len() as u64)],
        );
    }

    let started = crate::perf::start();
    calls::resolve_calls(facts);
    if let Some(started) = started {
        crate::perf::emit(
            "python.call_resolution",
            started.elapsed(),
            &[("calls", facts.calls.len() as u64)],
        );
    }
}
