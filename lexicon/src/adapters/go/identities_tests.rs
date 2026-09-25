use super::{
    discovery::Module,
    identities::{
        builtin, canonical_namespace, capture, closure, dynamic_method, function, import,
        interface_method, method, named_type, namespace, node_id, node_id_for_kind, package,
        positioned_symbol, ssa_function, test, type_expression,
    },
};

#[test]
fn canonical_identity_vectors_match_legacy_go_hashes() {
    let vectors = [
        (
            package("example.com/project", "project"),
            "sha256:3fc2bc6c9d17553d2c98ca9fdfd756bac9ceb36f7fd10a9cb128feb14acee5ec",
        ),
        (
            named_type("example.com/project", "Thing"),
            "sha256:180520d807e8c519337e0ab5057e790319efefd46fef38f65b7e27df0e9ab10b",
        ),
        (
            function("example.com/project", "Run"),
            "sha256:7c3c3ed9ad5d1d71df08aa1d8815d2823a845b18756348f53d95fe7d72cccd79",
        ),
        (
            method("example.com/project", "*Thing", "Run"),
            "sha256:3f989a2347b98296a2e034334137af838b81e648b92dfc502862b70f864e965a",
        ),
        (
            interface_method("example.com/project", "Runner", "Run"),
            "sha256:563390453f06382da76474572abe2e6dbfdde5734328f4047a87f95a9087a731",
        ),
        (
            closure("example.com/project", "main.go", 22, 13).unwrap(),
            "sha256:c7a537b5e9d1e8bade80cd2ee57c136898081cbebbf7791676f3414f52ec95b6",
        ),
        (
            positioned_symbol(
                "variable",
                "example.com/project",
                "main.go",
                21,
                2,
                "captured",
            )
            .unwrap(),
            "sha256:722c7cdaacae3a43f24f8d935f771a92cf84784a216aa00b7e442cca30bcf24e",
        ),
        (
            capture("sha256:deadbeef", 0, "value"),
            "sha256:e13d5605007dcb0a12895075d3be60c31bb1b3146e74600cbae458ac37967ee9",
        ),
        (
            ssa_function("example.com/project", "(*Thing).Run$bound", Some((12, 3))),
            "sha256:72cfb6e3a6bbb22a8ceca0a43d46257512ffe603a993d841ebefdfda92dbc6a9",
        ),
        (
            namespace("go:builtins"),
            "sha256:0b09d348981dcdd7690448328a95c26a30c553b927141f40f32acb2dc56c21ff",
        ),
        (
            builtin("len"),
            "sha256:ecc7a0a9b1fe22b78e0be22f0383832e5df47b2e1616f7b1a81d87ccb1356d45",
        ),
        (
            type_expression("[]int"),
            "sha256:8f19224a062772133a290d4ea0aa9b9d7589603485e1cb517ad061d03277a256",
        ),
        (
            dynamic_method("*Thing", "Run"),
            "sha256:3e3ed5701e173eae8b22988fe29ac9e7873516e963bdb5ceb7629c85392e695a",
        ),
        (
            test("example.com/project", "TestRun"),
            "sha256:381c2dddcf17a4e5e8b20bc291296bc6e6a9a517cb73b07a55f9b0017a6c1dab",
        ),
        (
            import("external", "fmt"),
            "sha256:884f8810821993e44a1be786b6fa2a99cb1fde1a12fd8230e01afda84ca37f37",
        ),
        (
            positioned_symbol("field", "example.com/project", "main.go", 4, 2, "Field").unwrap(),
            "sha256:32fc37150dc2baea80c6e906c1afd544ae1fd24728d0bf91d02d5cfb292ce6fb",
        ),
        (
            positioned_symbol(
                "constant",
                "example.com/project",
                "main.go",
                5,
                7,
                "Constant",
            )
            .unwrap(),
            "sha256:6d75a5b3a9607cf8ba2c3110376e3edb005818f0d48d5b3d6de46e3ed40dd679",
        ),
        (
            positioned_symbol(
                "parameter",
                "example.com/project",
                "main.go",
                7,
                10,
                "value",
            )
            .unwrap(),
            "sha256:c96599566da16086f3e38a7c4d6e90d3170b89878dcda9eabae31c91ad596dd4",
        ),
    ];
    for (identity, expected) in vectors {
        assert_eq!(node_id(&identity).unwrap(), expected, "{identity}");
    }
}

#[test]
fn canonicalizes_internal_test_namespaces_only() {
    let modules = vec![Module {
        root: ".".into(),
        path: "example.com/project".into(),
    }];
    assert_eq!(
        canonical_namespace(&modules, "example.com/project/tooling_test"),
        "example.com/project/tooling"
    );
    assert_eq!(
        canonical_namespace(&modules, "example.net/external_test"),
        "example.net/external_test"
    );
}

#[test]
fn rejects_wrong_kind_and_absolute_path_material() {
    assert!(node_id_for_kind("function:example.com/project:Run", "method").is_err());
    assert!(node_id("closure:example.com/project:/tmp/main.go:1:1").is_err());
    assert!(node_id(r"closure:example.com/project:C:\tmp\main.go:1:1").is_err());
    assert!(closure("example.com/project", "../main.go", 1, 1).is_err());
}
