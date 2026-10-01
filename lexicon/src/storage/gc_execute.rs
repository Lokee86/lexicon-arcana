use std::collections::BTreeSet;
use std::fs;

use super::export::operation;
use super::gc_storage::read_consumer_pins;
use super::gc_validate::{add_manifest_objects, canonical_plan};
use super::{GcPlan, GcResult, StorageError, Store};

impl Store {
    pub fn execute_gc(&self, plan: GcPlan, dry_run: bool) -> Result<GcResult, StorageError> {
        let _guard = self.lock()?;
        self.execute_gc_locked(plan, dry_run)
    }

    pub(super) fn execute_gc_locked(
        &self,
        plan: GcPlan,
        dry_run: bool,
    ) -> Result<GcResult, StorageError> {
        let plan = canonical_plan(plan)?;
        self.refuse_gc_during_pending()?;
        let (current, _) = self.current()?;
        if current != plan.current_snapshot {
            return Err(operation(format!(
                "Lexicon CURRENT changed during GC: planned {}, found {}",
                plan.current_snapshot, current
            )));
        }

        // A consumer pin or legacy bootstrap can appear after plan_gc.
        // Never execute a stale plan that would prune newly reachable data.
        for pinned in read_consumer_pins(self)? {
            if !plan.preserved_snapshots.contains(&pinned) {
                return Err(operation("consumer pins changed after GC planning"));
            }
        }
        let mut required_facts = BTreeSet::new();
        let mut required_topology = BTreeSet::new();
        for snapshot in &plan.preserved_snapshots {
            let manifest = self.load_snapshot(snapshot)?;
            add_manifest_objects(&mut required_facts, &manifest)?;
            self.add_topology_references(snapshot, &manifest, &mut required_topology)?;
        }
        let preserved_facts = plan.preserved_objects.iter().collect::<BTreeSet<_>>();
        let preserved_topology = plan
            .preserved_topology_objects
            .iter()
            .collect::<BTreeSet<_>>();
        if required_facts
            .iter()
            .any(|id| !preserved_facts.contains(id))
            || required_topology
                .iter()
                .any(|id| !preserved_topology.contains(id))
        {
            return Err(operation(
                "GC plan omits objects reachable from retained snapshots",
            ));
        }

        let mut result = GcResult {
            dry_run,
            deleted_snapshots: Vec::new(),
            deleted_objects: Vec::new(),
            deleted_topology_objects: Vec::new(),
            deleted_bootstrap_snapshots: Vec::new(),
        };
        if dry_run {
            result.deleted_snapshots = plan.delete_snapshots;
            result.deleted_objects = plan.delete_objects;
            result.deleted_topology_objects = plan.delete_topology_objects;
            result.deleted_bootstrap_snapshots = plan.delete_bootstrap_snapshots;
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
        for id in plan.delete_topology_objects {
            fs::remove_file(self.topology_path(&id))
                .map_err(|error| operation(format!("delete Lexicon topology {id}: {error}")))?;
            result.deleted_topology_objects.push(id);
        }
        for snapshot in plan.delete_bootstrap_snapshots {
            let directory = self
                .root()
                .join("topology/bootstrap")
                .join(snapshot.trim_start_matches("sha256:"));
            fs::remove_dir_all(&directory).map_err(|error| {
                operation(format!(
                    "delete legacy topology bootstrap {snapshot}: {error}"
                ))
            })?;
            result.deleted_bootstrap_snapshots.push(snapshot);
        }
        Ok(result)
    }
}
