use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use crate::ScanReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchOptions {
    pub debounce: Duration,
    pub reconcile: Duration,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(150),
            reconcile: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchSource {
    Startup,
    Watch,
    Reconcile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchNotice {
    Scan {
        source: WatchSource,
        report: ScanReport,
    },
    ScanError {
        source: WatchSource,
        error: String,
    },
    WatcherError {
        error: String,
    },
}

#[derive(Clone, Default)]
pub struct WatchStop(Arc<AtomicBool>);

impl WatchStop {
    pub fn stop(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
