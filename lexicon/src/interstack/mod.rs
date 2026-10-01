mod build;
mod config;
mod http;
mod http_consumers;
mod http_producers;
mod http_rails;
mod http_util;
mod message_constants;
mod messages;
mod model;
mod node_loading;
mod owner_ranges;
mod paths;
mod process;
mod process_commands;
mod protocol;
mod resolver;
mod source;
mod state;

use sha2::{Digest, Sha256};

pub use build::{interstack_drifted, refresh_interstack};
pub use model::{Library, Node};
pub use resolver::{ResolveResult, Summary, resolve};

pub const LANGUAGE: &str = "interstack";
pub const ADAPTER_VERSION: &str = "0.2.0";

pub fn adapter_fingerprint() -> String {
    let payload = [LANGUAGE, ADAPTER_VERSION, "1"].join("\0");
    let digest = Sha256::digest(payload.as_bytes());
    format!("sha256:{digest:x}")
}
