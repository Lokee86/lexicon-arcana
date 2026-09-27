use std::sync::LazyLock;

use regex::Regex;

use super::model::SourceFile;
use super::resolver::Resolver;

static CONSTANT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r#"^\s*const\s+([A-Za-z_][A-Za-z0-9_]*)[^=]*=\s*["']([^"']+)["']"#,
        r#"^\s*(?:pub(?:\([^)]*\))?\s+)?const\s+([A-Za-z_][A-Za-z0-9_]*)[^=]*=\s*["']([^"']+)["']"#,
        r#"^\s*(?:export\s+)?const\s+([A-Za-z_][A-Za-z0-9_]*)[^=]*=\s*["']([^"']+)["']"#,
        r#"^\s*([A-Z][A-Z0-9_]*)\s*=\s*["']([^"']+)["']"#,
    ]
    .into_iter()
    .map(|pattern| Regex::new(pattern).unwrap())
    .collect()
});

impl Resolver<'_> {
    pub(crate) fn collect_constants(&mut self, file: &SourceFile) {
        for line in &file.lines {
            for pattern in CONSTANT_PATTERNS.iter() {
                let Some(capture) = pattern.captures(line) else {
                    continue;
                };
                self.constants
                    .entry(capture[1].to_owned())
                    .or_default()
                    .insert(capture[2].to_owned());
            }
        }
    }
}
