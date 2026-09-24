use std::io::Write;
use std::path::PathBuf;

use crate::{ScanReport, run_consumers};

use super::{Lexicon, LexiconError};

impl Lexicon {
    pub fn scan(&self) -> Result<ScanReport, LexiconError> {
        let report = self.engine.scan()?;
        self.notify_consumers(&report, None)?;
        Ok(report)
    }

    pub fn scan_with_consumer_output(
        &self,
        output: &mut dyn Write,
    ) -> Result<ScanReport, LexiconError> {
        let report = self.engine.scan()?;
        self.notify_consumers(&report, Some(output))?;
        Ok(report)
    }

    pub fn scan_paths(&self, paths: &[PathBuf]) -> Result<ScanReport, LexiconError> {
        let report = self.engine.scan_paths(paths)?;
        self.notify_consumers(&report, None)?;
        Ok(report)
    }

    pub fn scan_paths_with_consumer_output(
        &self,
        paths: &[PathBuf],
        output: &mut dyn Write,
    ) -> Result<ScanReport, LexiconError> {
        let report = self.engine.scan_paths(paths)?;
        self.notify_consumers(&report, Some(output))?;
        Ok(report)
    }

    fn notify_consumers(
        &self,
        report: &ScanReport,
        output: Option<&mut dyn Write>,
    ) -> Result<(), LexiconError> {
        run_consumers(
            &self.repository,
            &self.state_root,
            &report.snapshot_id,
            output,
        )
    }
}
