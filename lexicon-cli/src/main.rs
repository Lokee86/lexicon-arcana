fn main() {
    std::process::exit(lexicon_cli::run(
        std::env::args().skip(1).collect(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ));
}
