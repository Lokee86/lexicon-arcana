use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};

use crate::{IgnorePolicy, Lexicon, LexiconError};

use super::event::{ignored, is_ignore_file, relevant};
use super::{WatchNotice, WatchOptions, WatchSource, WatchStop};

impl Lexicon {
    pub fn watch(
        &self,
        options: WatchOptions,
        stop: &WatchStop,
        mut notice: impl FnMut(WatchNotice),
    ) -> Result<(), LexiconError> {
        let options = normalized(options);
        let (sender, receiver) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })
        .map_err(watch_error)?;
        watcher
            .watch(self.repository(), RecursiveMode::Recursive)
            .map_err(watch_error)?;

        let mut policy = IgnorePolicy::load(self.repository())?;
        let startup = self.scan()?;
        notice(WatchNotice::Scan {
            source: WatchSource::Startup,
            report: startup,
        });

        let mut pending = BTreeSet::<PathBuf>::new();
        let mut full_scan = false;
        let mut debounce_at = None::<Instant>;
        let mut reconcile_at = Instant::now() + options.reconcile;

        while !stop.is_stopped() {
            let timeout = wait_duration(debounce_at, reconcile_at);
            match receiver.recv_timeout(timeout) {
                Ok(Ok(event)) => {
                    for path in &event.paths {
                        if is_ignore_file(self.repository(), path) {
                            policy = IgnorePolicy::load(self.repository())?;
                            pending.clear();
                            full_scan = true;
                            debounce_at = Some(Instant::now() + options.debounce);
                            continue;
                        }
                        if ignored(&policy, path) || !relevant(&event, path) {
                            continue;
                        }
                        pending.insert(path.clone());
                        debounce_at = Some(Instant::now() + options.debounce);
                    }
                }
                Ok(Err(error)) => {
                    notice(WatchNotice::WatcherError {
                        error: error.to_string(),
                    });
                    reconcile(self, WatchSource::Reconcile, &mut notice);
                    reconcile_at = Instant::now() + options.reconcile;
                }
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
                Err(RecvTimeoutError::Timeout) => {}
            }

            let now = Instant::now();
            if debounce_at.is_some_and(|deadline| now >= deadline) {
                debounce_at = None;
                let paths = pending.iter().cloned().collect::<Vec<_>>();
                pending.clear();
                if full_scan {
                    full_scan = false;
                    reconcile(self, WatchSource::Watch, &mut notice);
                } else if !paths.is_empty() {
                    scan_paths(self, paths, &mut notice);
                }
            }
            if now >= reconcile_at {
                reconcile(self, WatchSource::Reconcile, &mut notice);
                reconcile_at = now + options.reconcile;
            }
        }
        Ok(())
    }
}

fn scan_paths(lexicon: &Lexicon, paths: Vec<PathBuf>, notice: &mut impl FnMut(WatchNotice)) {
    match lexicon.scan_paths(&paths) {
        Ok(report) => notice(WatchNotice::Scan {
            source: WatchSource::Watch,
            report,
        }),
        Err(error) => notice(WatchNotice::ScanError {
            source: WatchSource::Watch,
            error: error.to_string(),
        }),
    }
}

fn reconcile(lexicon: &Lexicon, source: WatchSource, notice: &mut impl FnMut(WatchNotice)) {
    match lexicon.scan() {
        Ok(report) => notice(WatchNotice::Scan { source, report }),
        Err(error) => notice(WatchNotice::ScanError {
            source,
            error: error.to_string(),
        }),
    }
}

fn normalized(options: WatchOptions) -> WatchOptions {
    WatchOptions {
        debounce: positive(options.debounce, Duration::from_millis(150)),
        reconcile: positive(options.reconcile, Duration::from_secs(30)),
    }
}

fn positive(value: Duration, fallback: Duration) -> Duration {
    if value.is_zero() { fallback } else { value }
}

fn wait_duration(debounce: Option<Instant>, reconcile: Instant) -> Duration {
    let now = Instant::now();
    let next = debounce.map_or(reconcile, |deadline| deadline.min(reconcile));
    next.saturating_duration_since(now)
        .min(Duration::from_millis(100))
}

fn watch_error(error: impl ToString) -> LexiconError {
    LexiconError::new(error.to_string())
}
