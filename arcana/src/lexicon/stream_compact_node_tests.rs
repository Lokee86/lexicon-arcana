use super::binary_v2_reader::SpanRef;
use super::binary_v2_stream::NodeRef;
use super::identity::LexiconIdentity;
use super::stream_compact_node::signature_digest;

fn digest(
    attributes: Option<&[u8]>,
    content_id: Option<[u8; 32]>,
    kind: &str,
    name: &str,
    owner: Option<&str>,
    path: &str,
    qualified_name: &str,
    span: Option<SpanRef<'_>>,
) -> [u8; 32] {
    signature_digest(&NodeRef {
        attributes,
        content_id: content_id.map(LexiconIdentity::from_digest),
        id: LexiconIdentity::from_digest([9; 32]),
        kind,
        name,
        owner,
        path,
        qualified_name,
        span,
    })
}

fn test_span(path: &str) -> SpanRef<'_> {
    SpanRef {
        path,
        start_line: 10,
        start_column: 20,
        end_line: 30,
        end_column: 40,
    }
}

fn base_digest(span: Option<SpanRef<'_>>) -> [u8; 32] {
    digest(
        Some(b"{\"x\":1}"),
        Some([3; 32]),
        "function",
        "name",
        Some("src/file.rs"),
        "src/file.rs",
        "module::name",
        span,
    )
}

#[test]
fn signature_digest_covers_all_duplicate_semantics() {
    let base = base_digest(Some(test_span("src/file.rs")));

    let changed = [
        digest(
            Some(b"{\"x\":2}"),
            Some([3; 32]),
            "function",
            "name",
            Some("src/file.rs"),
            "src/file.rs",
            "module::name",
            Some(test_span("src/file.rs")),
        ),
        digest(
            Some(b"{\"x\":1}"),
            Some([4; 32]),
            "function",
            "name",
            Some("src/file.rs"),
            "src/file.rs",
            "module::name",
            Some(test_span("src/file.rs")),
        ),
        digest(
            Some(b"{\"x\":1}"),
            Some([3; 32]),
            "method",
            "name",
            Some("src/file.rs"),
            "src/file.rs",
            "module::name",
            Some(test_span("src/file.rs")),
        ),
        digest(
            Some(b"{\"x\":1}"),
            Some([3; 32]),
            "function",
            "other",
            Some("src/file.rs"),
            "src/file.rs",
            "module::name",
            Some(test_span("src/file.rs")),
        ),
        digest(
            Some(b"{\"x\":1}"),
            Some([3; 32]),
            "function",
            "name",
            Some("src/other.rs"),
            "src/file.rs",
            "module::name",
            Some(test_span("src/file.rs")),
        ),
        digest(
            Some(b"{\"x\":1}"),
            Some([3; 32]),
            "function",
            "name",
            Some("src/file.rs"),
            "src/other.rs",
            "module::name",
            Some(test_span("src/file.rs")),
        ),
        digest(
            Some(b"{\"x\":1}"),
            Some([3; 32]),
            "function",
            "name",
            Some("src/file.rs"),
            "src/file.rs",
            "module::other",
            Some(test_span("src/file.rs")),
        ),
    ];
    assert!(changed.into_iter().all(|digest| digest != base));

    for span in [
        SpanRef {
            path: "src/other.rs",
            ..test_span("src/file.rs")
        },
        SpanRef {
            start_line: 11,
            ..test_span("src/file.rs")
        },
        SpanRef {
            start_column: 21,
            ..test_span("src/file.rs")
        },
        SpanRef {
            end_line: 31,
            ..test_span("src/file.rs")
        },
        SpanRef {
            end_column: 41,
            ..test_span("src/file.rs")
        },
    ] {
        assert_ne!(base, base_digest(Some(span)));
    }
    assert_ne!(base, base_digest(None));
}

#[test]
fn signature_digest_frames_optional_and_adjacent_bytes_unambiguously() {
    let absent = digest(None, None, "ab", "c", None, "d", "e", None);
    let empty = digest(Some(b""), None, "ab", "c", None, "d", "e", None);
    let repartitioned = digest(None, None, "a", "bc", None, "d", "e", None);
    let empty_owner = digest(None, None, "ab", "c", Some(""), "d", "e", None);

    assert_ne!(absent, empty);
    assert_ne!(absent, repartitioned);
    assert_ne!(absent, empty_owner);
    assert_eq!(absent, digest(None, None, "ab", "c", None, "d", "e", None));
}
