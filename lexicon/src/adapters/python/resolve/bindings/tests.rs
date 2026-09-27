use super::resolve_imports;
use crate::adapters::python::facts::Facts;
use crate::adapters::python::model::ImportInfo;

#[test]
fn reexport_resolution_requeues_only_affected_imports() {
    let mut facts = Facts::new("fixture".into());
    for module in ["pkg.a", "pkg.b", "pkg.c"] {
        let id = facts.add_node(
            "module",
            module.rsplit('.').next().unwrap_or(module),
            &format!("{}.py", module.replace('.', "/")),
            module,
            Some(module),
            None,
            None,
            None,
        );
        facts.modules.insert(module.into(), id);
    }
    let value = facts.add_node(
        "variable",
        "value",
        "pkg/c.py",
        "pkg.c.value",
        Some("pkg.c.value"),
        None,
        None,
        None,
    );
    facts.symbols.insert("pkg.c.value".into(), value.clone());

    let a = facts.modules["pkg.a"].clone();
    let b = facts.modules["pkg.b"].clone();
    facts.imports = vec![
        import(&a, "pkg.a", "b", "value"),
        import(&b, "pkg.b", "c", "value"),
    ];

    resolve_imports(&mut facts);

    assert_eq!(
        facts.module_bindings.get(&("pkg.b".into(), "value".into())),
        Some(&(Some(value.clone()), String::new()))
    );
    assert_eq!(
        facts.module_bindings.get(&("pkg.a".into(), "value".into())),
        Some(&(Some(value), String::new()))
    );
}

fn import(owner_id: &str, module_name: &str, target_module: &str, target_name: &str) -> ImportInfo {
    ImportInfo {
        module_name: module_name.into(),
        owner_id: owner_id.into(),
        expression: format!("from .{target_module} import {target_name}"),
        binding: Some(target_name.into()),
        target_module: target_module.into(),
        target_name: Some(target_name.into()),
        relative_level: 1,
        star: false,
        is_package: false,
        span: None,
    }
}
