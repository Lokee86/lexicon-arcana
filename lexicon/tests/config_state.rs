mod support;

use lexicon::{
    ANALYSIS_CONFIG_ID, Config, find_adapter_root_from, load_config, normalize_enabled_languages,
    save_config, save_config_with_languages, update_enabled_languages,
};
use std::fs;

use support::TestDirectory;

#[test]
fn analysis_config_identity_matches_go() {
    assert_eq!(
        ANALYSIS_CONFIG_ID,
        "sha256:3eed35c601d346d8b9ebeef46b5e98e5e6307192a8b7676d56cf148f4034ca8a"
    );
}

#[test]
fn enabled_languages_match_go_normalization_and_defaults() {
    assert_eq!(
        normalize_enabled_languages(&["typescript".into(), "python".into(), "python".into(),])
            .unwrap(),
        vec!["python", "typescript"]
    );
    assert!(normalize_enabled_languages(&["python".into(), "klingon".into()]).is_err());

    let default = Config::default();
    assert!(default.language_enabled("python"));
    assert!(default.language_enabled("typescript"));
    assert!(!default.language_enabled("generic-java"));

    let generic = Config {
        enabled_languages: vec!["generic".into()],
        ..Default::default()
    };
    assert!(generic.language_enabled("generic-scala"));
    assert!(
        !Config {
            enabled_languages: vec!["python".into()],
            ..Default::default()
        }
        .language_enabled("generic-scala")
    );
}

#[test]
fn save_load_and_update_preserve_enabled_selection() {
    let repository = TestDirectory::new("config");
    let adapter_root = repository.path.join("adapters");
    fs::create_dir_all(&adapter_root).unwrap();

    save_config_with_languages(
        &repository.path,
        &adapter_root,
        &["typescript".into(), "python".into(), "python".into()],
    )
    .unwrap();
    let loaded = load_config(&repository.path).unwrap();
    assert_eq!(loaded.enabled_languages, vec!["python", "typescript"]);

    let replacement = repository.path.join("replacement-adapters");
    save_config(&repository.path, &replacement).unwrap();
    assert_eq!(
        load_config(&repository.path).unwrap().enabled_languages,
        vec!["python", "typescript"]
    );

    update_enabled_languages(&repository.path, &[]).unwrap();
    let loaded = load_config(&repository.path).unwrap();
    assert!(loaded.enabled_languages.is_empty());
    assert!(loaded.language_enabled("ruby"));
}

#[test]
fn legacy_config_without_enabled_languages_defaults_to_all() {
    let repository = TestDirectory::new("legacy-config");
    let state = repository.path.join(".lexicon");
    fs::create_dir_all(&state).unwrap();
    fs::write(
        state.join("config.json"),
        br#"{"version":1,"adapter_root":"adapters"}"#,
    )
    .unwrap();

    let loaded = load_config(&repository.path).unwrap();
    assert!(loaded.enabled_languages.is_empty());
    assert!(loaded.language_enabled("go"));
}

#[test]
fn adapter_root_search_order_matches_go_bundle_rules() {
    let root = TestDirectory::new("adapter-root");
    let bundle = root.path.join("bundle");
    let sibling = bundle.join("adapters");
    fs::create_dir_all(sibling.join("python")).unwrap();

    let found = find_adapter_root_from(
        &root.path,
        None,
        None,
        Some(&bundle.join("bin").join("lexicon.exe")),
        Some(&root.path.join("cwd")),
    )
    .unwrap();
    assert_eq!(found, sibling);
}
