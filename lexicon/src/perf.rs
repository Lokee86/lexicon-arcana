use std::time::{Duration, Instant};

const PERFORMANCE_ENVIRONMENT: &str = "LEXICON_PERF";

pub(crate) fn enabled() -> bool {
    let Some(value) = std::env::var_os(PERFORMANCE_ENVIRONMENT) else {
        return false;
    };
    value_enabled(&value.to_string_lossy())
}

fn value_enabled(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "off" | "no"
    )
}

pub(crate) fn start() -> Option<Instant> {
    enabled().then(Instant::now)
}

pub(crate) fn emit(stage: &str, elapsed: Duration, counters: &[(&str, u64)]) {
    if !enabled() {
        return;
    }

    eprint!(
        "[lexicon-perf] stage={stage} elapsed_ms={:.3}",
        elapsed.as_secs_f64() * 1000.0
    );
    for (name, value) in counters {
        eprint!(" {name}={value}");
    }
    eprintln!();
}

#[cfg(test)]
mod tests {
    use super::value_enabled;

    #[test]
    fn disabled_values_are_recognized() {
        for value in ["", "0", "false", "FALSE", "off", "no"] {
            assert!(!value_enabled(value));
        }
        assert!(value_enabled("1"));
        assert!(value_enabled("true"));
    }
}
