use std::env;
use std::path::Path;
use std::time::Instant;

use lexicon::Store;
use lexicon::interstack::refresh_interstack;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state_root = env::args()
        .nth(1)
        .ok_or("usage: interstack_profile <state-root>")?;
    let state_root = Path::new(&state_root);
    let store = Store::new(state_root);
    let (_, manifest) = store.current()?;

    let started = Instant::now();
    let (refreshed, _) =
        refresh_interstack(&store, &state_root.join("repo").join("source"), manifest)?;
    println!(
        "{}",
        json!({
            "wall_ms": started.elapsed().as_secs_f64() * 1000.0,
            "languages": refreshed.languages.as_deref().unwrap_or_default().len(),
        })
    );
    Ok(())
}
