use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::LexiconError;

use super::atomic::write_atomic;

pub const CONSUMER_STATE_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumerSuccessState {
    pub version: u64,
    pub snapshot_id: String,
}

pub(super) fn save_snapshot(
    state_root: &Path,
    name: &str,
    snapshot_id: &str,
) -> Result<(), LexiconError> {
    let mut data = serde_json::to_vec_pretty(&ConsumerSuccessState {
        version: CONSUMER_STATE_VERSION,
        snapshot_id: snapshot_id.to_owned(),
    })
    .map_err(|error| LexiconError::new(format!("encode consumer state: {error}")))?;
    data.push(b'\n');
    write_atomic(&state_root.join("consumer-state").join(name), &data)
}
