use std::env;
use std::path::Path;
use std::time::Instant;

use lexicon::repository::SourceMirror;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = env::args()
        .nth(1)
        .ok_or("usage: mirror_profile <source-root> <state-root>")?;
    let state = env::args()
        .nth(2)
        .ok_or("usage: mirror_profile <source-root> <state-root>")?;
    let mirror = SourceMirror::new(Path::new(&state).join("repo").join("source"));

    let started = Instant::now();
    mirror.sync_all(Path::new(&source))?;
    println!(
        "{}",
        json!({
            "wall_ms": started.elapsed().as_secs_f64() * 1000.0,
        })
    );
    Ok(())
}
