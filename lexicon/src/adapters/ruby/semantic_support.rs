fn node_exists_q(records: &[FactRecord], q: &str) -> bool {
    records
        .iter()
        .any(|r| matches!(r, FactRecord::Node(n) if n.qualified_name == q))
}
fn node_kind_q<'a>(records: &'a [FactRecord], q: &str) -> Option<&'a str> {
    records.iter().find_map(|r| match r {
        FactRecord::Node(n) if n.qualified_name == q => Some(n.kind.as_str()),
        _ => None,
    })
}
fn node_exists_id(records: &[FactRecord], id: &str) -> bool {
    records
        .iter()
        .any(|r| matches!(r, FactRecord::Node(n) if n.id == id))
}
fn qualified_for(records: &[FactRecord], id: &str) -> Option<String> {
    records.iter().find_map(|r| match r {
        FactRecord::Node(n) if n.id == id => Some(n.qualified_name.clone()),
        _ => None,
    })
}
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
fn is_non_call_keyword(token: &str) -> bool {
    matches!(
        token,
        "if" | "unless"
            | "while"
            | "until"
            | "do"
            | "end"
            | "return"
            | "class"
            | "module"
            | "def"
            | "include"
            | "module_function"
            | "true"
            | "false"
            | "nil"
            | "then"
            | "else"
            | "elsif"
            | "rescue"
            | "ensure"
            | "begin"
            | "yield"
    )
}
fn collect(root: &Path, dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                let n = e.file_name().to_string_lossy().to_string();
                if ![
                    ".git",
                    ".worktrees",
                    ".workingtrees",
                    ".ddocs",
                    ".lexicon",
                    ".arcana",
                    ".grimoire",
                    ".pitlord",
                    ".cantrip",
                    ".homunculus",
                    ".incubus",
                    ".ritual",
                    ".warlock",
                    ".bundle",
                    "vendor",
                    "node_modules",
                    "target",
                    "build",
                    "dist",
                    "tmp",
                    "log",
                    "coverage",
                ]
                .contains(&n.as_str())
                {
                    collect(root, &p, out);
                }
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("rb")) {
                out.push(p);
            }
        }
    }
    let _ = root;
}
