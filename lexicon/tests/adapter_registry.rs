use lexicon::{
    ADAPTER_CONTRACT_VERSION, AdapterContract,
    languages::{definitions, lookup, supports_partitioned_execution},
};

#[test]
fn definitions_match_language_directories_and_capabilities() {
    let definitions = definitions();
    assert_eq!(definitions.len(), 12);

    let go = lookup("go").unwrap();
    assert_eq!(go.directory, "go");
    assert!(go.partitioned_execution);

    let python = lookup("python").unwrap();
    assert_eq!(python.directory, "python");
    assert!(python.partitioned_execution);

    let typescript = lookup("typescript").unwrap();
    assert_eq!(typescript.directory, "typescript");
    assert!(typescript.extensions.contains(&".js".into()));
    assert!(typescript.extensions.contains(&".svelte".into()));

    let generic = lookup("generic-scala").unwrap();
    assert_eq!(generic.directory, "generic");
    assert_eq!(generic.extensions, vec![".scala"]);
    assert!(!supports_partitioned_execution("generic-scala"));
}

#[test]
fn current_adapter_contract_is_explicit_and_versioned() {
    assert_eq!(AdapterContract::CURRENT.version, ADAPTER_CONTRACT_VERSION);
    assert_eq!(
        AdapterContract::CURRENT.fact_schema_version,
        lexicon::FACT_SCHEMA_VERSION
    );
}
