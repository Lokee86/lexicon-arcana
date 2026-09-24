use std::fs;

use super::export::operation;
use super::gc_validate::canonical_plan;
use super::{GcPlan, GcResult, StorageError, Store};

impl Store {
    pub fn execute_gc(&self, plan: GcPlan, dry_run: bool) -> Result<GcResult, StorageError> {
        let plan = canonical_plan(plan)?;
        let (current, _) = self.current()?;
        if current != plan.current_snapshot {
            return Err(operation(format!(
                "Lexicon CURRENT changed during GC: planned {}, found {}",
                plan.current_snapshot, current
            )));
        }

        let mut result = GcResult {
            dry_run,
            deleted_snapshots: Vec::new(),
            deleted_objects: Vec::new(),
        };
        if dry_run {
            result.deleted_snapshots = plan.delete_snapshots;
            result.deleted_objects = plan.delete_objects;
            return Ok(result);
        }

        for id in plan.delete_snapshots {
            fs::remove_file(self.snapshot_path(&id))
                .map_err(|error| operation(format!("delete Lexicon snapshot {id}: {error}")))?;
            result.deleted_snapshots.push(id);
        }
        for id in plan.delete_objects {
            fs::remove_file(self.object_path(&id))
                .map_err(|error| operation(format!("delete Lexicon object {id}: {error}")))?;
            result.deleted_objects.push(id);
        }
        Ok(result)
    }
}
