mod args;
mod commands_consumer;
mod commands_diagnostics;
mod commands_lookup;
mod commands_scan;
mod commands_storage;
mod format;
mod repository;

use std::io::Write;

pub fn run(arguments: Vec<String>, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let Some(command) = arguments.first() else {
        usage(stderr);
        return 2;
    };
    let tail = &arguments[1..];
    let result = match command.as_str() {
        "init" => commands_scan::init(tail, stdout),
        "scan" => commands_scan::scan(tail, stdout),
        "demon" => commands_scan::demon(tail, stdout, stderr),
        "rebuild" => commands_scan::rebuild(tail, stdout),
        "export" => commands_storage::export(tail, stdout),
        "gc" => commands_storage::gc(tail, stdout),
        "languages" => commands_storage::languages(tail, stdout),
        "consumer" => commands_consumer::consumer(tail, stdout),
        "status" => commands_diagnostics::status(tail, stdout),
        "doctor" => commands_diagnostics::doctor(tail, stdout),
        "find" => commands_lookup::find(tail, stdout),
        "show" => commands_lookup::show(tail, stdout),
        "refs" => commands_lookup::refs(tail, stdout),
        "calls" => commands_lookup::calls(tail, stdout),
        "version" => version(stdout),
        "help" | "-h" | "--help" => {
            usage(stdout);
            return 0;
        }
        _ => {
            let _ = writeln!(stderr, "unknown command {command:?}");
            usage(stderr);
            return 2;
        }
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            1
        }
    }
}

fn version(output: &mut dyn Write) -> Result<(), String> {
    let version = option_env!("LEXICON_VERSION").unwrap_or("dev");
    writeln!(output, "lexicon version {version}").map_err(|error| error.to_string())
}

fn usage(output: &mut dyn Write) {
    let _ = writeln!(
        output,
        "Usage: lexicon <init|scan|demon|rebuild|export|gc|languages|consumer|status|doctor|find|show|refs|calls|version> [options]"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_and_unknown_commands_keep_go_exit_contract() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run(Vec::new(), &mut stdout, &mut stderr), 2);
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("Usage: lexicon")
        );

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run(vec!["help".into()], &mut stdout, &mut stderr), 0);
        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("Usage: lexicon")
        );

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run(vec!["wat".into()], &mut stdout, &mut stderr), 2);
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("unknown command"));
        assert!(stderr.contains("Usage: lexicon"));
    }

    #[test]
    fn version_defaults_to_go_dev_value() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run(vec!["version".into()], &mut stdout, &mut stderr), 0);
        assert_eq!(String::from_utf8(stdout).unwrap(), "lexicon version dev\n");
    }
}
