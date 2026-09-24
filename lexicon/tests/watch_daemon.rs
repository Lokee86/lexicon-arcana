mod support;

use std::sync::Arc;
use std::time::Duration;

use lexicon::{AdapterHost, Lexicon, WatchNotice, WatchOptions, WatchSource, WatchStop};

use support::TestDirectory;
use support::scan_adapter::{FixtureAdapter, write};

#[test]
fn watch_performs_startup_scan_and_honors_stop_token() {
    let root = TestDirectory::new("watch-startup");
    let repository = root.path.join("repository");
    let adapter_root = root.path.join("adapters");
    write(&repository, "a.py", "value = 1\n");
    write(&adapter_root, "python/adapter.py", "version = 1\n");

    let adapter = Arc::new(FixtureAdapter::new(false));
    let mut host = AdapterHost::new(&adapter_root);
    host.register_native("python", adapter);
    let (lexicon, initialized) = Lexicon::initialize_with_host(&repository, host).unwrap();

    let stop = WatchStop::default();
    let signal = stop.clone();
    let mut notices = Vec::new();
    lexicon
        .watch(
            WatchOptions {
                debounce: Duration::from_millis(10),
                reconcile: Duration::from_millis(50),
            },
            &stop,
            |notice| {
                notices.push(notice);
                signal.stop();
            },
        )
        .unwrap();

    assert!(stop.is_stopped());
    assert_eq!(notices.len(), 1);
    match &notices[0] {
        WatchNotice::Scan { source, report } => {
            assert_eq!(*source, WatchSource::Startup);
            assert_eq!(report.snapshot_id, initialized.snapshot_id);
            assert!(report.changed.is_empty());
        }
        other => panic!("unexpected notice: {other:?}"),
    }
}
