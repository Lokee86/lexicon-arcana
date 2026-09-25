pub(super) mod bindings;
mod calls;
mod relationships;
mod shapes;

use super::facts::Facts;

pub fn resolve(facts: &mut Facts) {
    bindings::resolve_imports(facts);
    relationships::resolve_inheritance(facts);
    relationships::emit_overrides(facts);
    calls::resolve_calls(facts);
}
