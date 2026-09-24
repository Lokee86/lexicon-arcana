mod support;

use std::sync::Arc;

use lexicon::{AdapterHost, Lexicon};

use support::TestDirectory;
use support::scan_adapter::{FixtureAdapter, write};

#[test]
fn rebuild_without_languages_rebuilds_all_selected_source_languages() {
    let root = TestDirectory::new("rebuild-all");
    let repository = root.path.join("repository");
    let adapters = root.path.join("adapters");
    write(&repository, "main.py", "value = 1\n");
    write(&repository, "main.rb", "value = 1\n");
    write(&adapters, "python/adapter.py", "version = 1\n");
    write(&adapters, "ruby/adapter.rb", "version = 1\n");

    let python = Arc::new(FixtureAdapter::new(false));
    let ruby = Arc::new(FixtureAdapter::new(false));
    let mut host = AdapterHost::new(&adapters);
    host.register("python", python.clone());
    host.register("ruby", ruby.clone());

    let (lexicon, _) = Lexicon::initialize_with_host(&repository, host).unwrap();
    python.requests.lock().unwrap().clear();
    ruby.requests.lock().unwrap().clear();

    let report = lexicon.rebuild(&[]).unwrap();
    assert_eq!(report.languages, vec!["python", "ruby"]);
    assert_eq!(*python.requests.lock().unwrap(), vec![false]);
    assert_eq!(*ruby.requests.lock().unwrap(), vec![false]);
    assert!(!report.snapshot_id.is_empty());
}

#[test]
fn rebuild_rejects_disabled_explicit_language() {
    let root = TestDirectory::new("rebuild-disabled");
    let repository = root.path.join("repository");
    let adapters = root.path.join("adapters");
    write(&repository, "main.py", "value = 1\n");
    write(&repository, "main.rb", "value = 1\n");
    write(&adapters, "python/adapter.py", "version = 1\n");
    write(&adapters, "ruby/adapter.rb", "version = 1\n");

    let python = Arc::new(FixtureAdapter::new(false));
    let ruby = Arc::new(FixtureAdapter::new(false));
    let mut host = AdapterHost::new(&adapters);
    host.register("python", python);
    host.register("ruby", ruby);

    let enabled = vec!["python".to_owned()];
    let (lexicon, _) =
        Lexicon::initialize_with_host_and_languages(&repository, host, &enabled).unwrap();

    let error = lexicon.rebuild(&["ruby".to_owned()]).unwrap_err();
    assert!(error.to_string().contains("disabled"));
}
