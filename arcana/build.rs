use std::env;

fn main() {
    let version = env::var("ARCANA_RELEASE_VERSION").unwrap_or_else(|_| {
        env::var("CARGO_PKG_VERSION").expect("Cargo must provide CARGO_PKG_VERSION")
    });
    println!("cargo:rustc-env=ARCANA_RELEASE_VERSION={version}");
    println!("cargo:rerun-if-env-changed=ARCANA_RELEASE_VERSION");
}
