use sha2::{Digest, Sha256};

use crate::config::CONFIG_VERSION;

use super::{ADAPTER_CONTRACT_VERSION, AdapterError, LanguageAdapter};

pub(crate) fn adapter_fingerprint(
    language: &str,
    adapter: &dyn LanguageAdapter,
) -> Result<String, AdapterError> {
    let contract = adapter.contract();
    if contract.version != ADAPTER_CONTRACT_VERSION {
        return Err(AdapterError::new(format!(
            "{language} adapter contract version {} is unsupported; expected {}",
            contract.version, ADAPTER_CONTRACT_VERSION
        )));
    }

    let mut hash = Sha256::new();
    write_field(&mut hash, b"lexicon:native-adapter-fingerprint:v1");
    write_field(&mut hash, language.as_bytes());
    write_field(&mut hash, contract.version.to_string().as_bytes());
    write_field(
        &mut hash,
        contract.fact_schema_version.to_string().as_bytes(),
    );
    write_field(&mut hash, CONFIG_VERSION.to_string().as_bytes());
    write_field(&mut hash, adapter.implementation_version().as_bytes());
    write_field(&mut hash, adapter.implementation_fingerprint().as_bytes());
    Ok(format!("sha256:{:x}", hash.finalize()))
}

pub(crate) fn source_fingerprint(
    implementation_version: &str,
    sources: &[(&str, &[u8])],
) -> String {
    let mut hash = Sha256::new();
    write_field(&mut hash, b"lexicon:native-adapter-source:v1");
    write_field(&mut hash, implementation_version.as_bytes());
    for (path, bytes) in sources {
        write_field(&mut hash, path.as_bytes());
        write_field(&mut hash, bytes);
    }
    format!("sha256:{:x}", hash.finalize())
}

fn write_field(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}
