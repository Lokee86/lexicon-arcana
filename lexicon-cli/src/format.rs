use std::io::Write;

use lexicon::{ScanReport, WatchNotice, WatchSource};

pub fn parse_language_selection(value: &str) -> Result<Vec<String>, String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("all") {
        return Ok(Vec::new());
    }
    let mut languages = Vec::new();
    for part in value.split(',') {
        let language = part.trim().to_ascii_lowercase();
        if language.is_empty() {
            return Err("language list contains an empty value".into());
        }
        languages.push(language);
    }
    lexicon::normalize_enabled_languages(&languages)
}

pub fn parse_optional_languages(value: Option<&str>) -> Result<Vec<String>, String> {
    match value {
        Some(value) if !value.trim().is_empty() => parse_language_selection(value),
        _ => Ok(Vec::new()),
    }
}

pub fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".into()
    } else {
        values.join(", ")
    }
}

pub fn display_language_selection(values: &[String]) -> String {
    if values.is_empty() {
        "all".into()
    } else {
        values.join(", ")
    }
}

pub fn write_scan_report(output: &mut dyn Write, report: &ScanReport) -> Result<(), String> {
    if report.changed.is_empty() {
        if report.languages.is_empty() {
            writeln!(output, "Lexicon is current: {}", report.snapshot_id)
                .map_err(|error| error.to_string())?;
        } else {
            writeln!(output, "rebuilt libraries: {}", report.languages.join(", "))
                .map_err(|error| error.to_string())?;
            writeln!(output, "snapshot: {}", report.snapshot_id)
                .map_err(|error| error.to_string())?;
        }
    } else {
        writeln!(
            output,
            "updated {} files: {}",
            report.changed.len(),
            report.languages.join(", ")
        )
        .map_err(|error| error.to_string())?;
        writeln!(output, "snapshot: {}", report.snapshot_id).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn write_watch_notice(output: &mut dyn Write, notice: WatchNotice) {
    match notice {
        WatchNotice::Scan { source, report } if !report.changed.is_empty() => {
            let source = match source {
                WatchSource::Startup => "startup",
                WatchSource::Watch => "watch",
                WatchSource::Reconcile => "reconcile",
            };
            let _ = writeln!(
                output,
                "{source} scan: {} files, {}",
                report.changed.len(),
                report.languages.join(", ")
            );
        }
        WatchNotice::Scan { .. } => {}
        WatchNotice::ScanError {
            source: WatchSource::Watch,
            error,
        } => {
            let _ = writeln!(output, "lexicon demon scan failed: {error}");
        }
        WatchNotice::ScanError { error, .. } => {
            let _ = writeln!(output, "lexicon reconciliation failed: {error}");
        }
        WatchNotice::WatcherError { error } => {
            let _ = writeln!(output, "lexicon watcher error: {error}; reconciling");
        }
    }
}

pub fn consumer_file_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty()
        || std::path::Path::new(name).components().count() != 1
        || name.contains('/')
        || name.contains("\\")
    {
        return Err(format!("invalid consumer name {name:?}"));
    }
    if std::path::Path::new(name).extension().is_none() {
        Ok(format!("{name}.json"))
    } else {
        Ok(name.to_owned())
    }
}
