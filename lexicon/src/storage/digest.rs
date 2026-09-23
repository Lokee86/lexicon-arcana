use sha2::{Digest, Sha256};

const OBJECT_DOMAIN: &[u8] = b"lexicon:fact-object:v1\0";

pub fn object_id(encoded_object: &[u8]) -> String {
    domain_id(OBJECT_DOMAIN, encoded_object)
}

pub(crate) fn domain_id(domain: &[u8], data: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(data);
    format!("sha256:{:x}", hash.finalize())
}
