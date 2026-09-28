use super::{discovery::Module, semantic_identity_policy};

fn modules() -> Vec<Module> {
    vec![Module {
        root: ".".into(),
        path: "example.com/app".into(),
    }]
}

#[test]
fn reconstructs_canonical_identity_from_frontend_key() {
    assert_eq!(
        semantic_identity_policy::canonical_identity(
            &modules(),
            "method:example.com/app_test:Thing.Run"
        )
        .unwrap(),
        "method:example.com/app:Thing.Run"
    );
    assert_eq!(
        semantic_identity_policy::canonical_identity(
            &modules(),
            "variable:example.com/app_test:main.go:7:3:value"
        )
        .unwrap(),
        "variable:example.com/app:main.go:7:3:value"
    );
}

#[test]
fn reconstructs_ssa_target_from_namespace_evidence() {
    assert_eq!(
        semantic_identity_policy::canonical_target_identity(
            &modules(),
            "ssa-function:example.com/app:callback:9:4",
            Some("example.com/app")
        )
        .unwrap(),
        "ssa-function:example.com/app:callback:9:4"
    );
}
