use lexicon::languages::{for_path, language_enabled, owns_source};

#[test]
fn typescript_owns_existing_javascript_and_typescript_extensions() {
    for path in [
        "src/app.js",
        "src/view.jsx",
        "src/config.mjs",
        "src/legacy.cjs",
        "src/app.ts",
        "src/view.tsx",
        "src/config.mts",
        "src/legacy.cts",
        "src/component.svelte",
    ] {
        assert!(
            owns_source("typescript", path),
            "missing ownership for {path}"
        );
        assert_eq!(for_path(path), vec!["typescript"]);
    }
    assert!(!owns_source("typescript", "src/page.astro"));
}

#[test]
fn config_files_trigger_languages_without_becoming_sources() {
    assert_eq!(for_path("pyproject.toml"), vec!["python"]);
    assert!(!owns_source("python", "pyproject.toml"));
    assert_eq!(for_path("Cargo.toml"), vec!["rust"]);
    assert!(!owns_source("rust", "Cargo.toml"));
}

#[test]
fn generic_enablement_matches_go_registry_behavior() {
    assert_eq!(for_path("scripts/build.ps1"), vec!["generic-ps1"]);
    assert!(owns_source("generic-ps1", "scripts/build.ps1"));
    assert!(language_enabled("generic-ps1", &["generic".into()]));
    assert!(!language_enabled("generic-ps1", &["python".into()]));
}
