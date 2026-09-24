use std::io::Write;

use super::{AdapterError, AdapterRequest};

pub trait NativeAdapter: Send + Sync {
    fn run(&self, request: &AdapterRequest, output: &mut dyn Write) -> Result<(), AdapterError>;
}
