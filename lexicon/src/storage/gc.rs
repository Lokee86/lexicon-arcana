use std::collections::BTreeSet;

use super::gc_storage::{list_objects, list_snapshots, read_consumer_pins};
use super::gc_validate::add_manifest_objects;
use super::{StorageError, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GcOptions {
    pub keep_snapshots: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcPlan {
    pub current_snapshot: String,
    pub preserved_snapshots: Vec<String>,
    pub delete_snapshots: Vec<String>,
    pub preserved_objects: Vec<String>,
    pub delete_objects: Vec<String>,
    pub preserved_topology_objects: Vec<String>,
    pub delete_topology_objects: Vec<String>,
    pub delete_bootstrap_snapshots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcResult {
    pub dry_run: bool,
    pub deleted_snapshots: Vec<String>,
    pub deleted_objects: Vec<String>,
    pub deleted_topology_objects: Vec<String>,
    pub deleted_bootstrap_snapshots: Vec<String>,
}

impl Store {
    pub fn plan_gc(&self, options: GcOptions) -> Result<GcPlan, StorageError> {
        self.refuse_gc_during_pending()?;
        let (current_id, current) = self.current()?;
        let snapshots = list_snapshots(self)?;
        let pins = read_consumer_pins(self)?;

        let mut preserved = BTreeSet::from([current_id.clone()]);
        preserved.extend(
            snapshots
                .iter()
                .take(options.keep_snapshots)
                .map(|snapshot| snapshot.id.clone()),
        );
        preserved.extend(pins);

        let mut manifests = std::collections::BTreeMap::new();
        manifests.insert(current_id.clone(), current);
        let mut objects = BTreeSet::new();
        let mut topology = BTreeSet::new();
        for id in &preserved {
            let manifest = match manifests.remove(id) {
                Some(manifest) => manifest,
                None => self.load_snapshot(id).map_err(|error| {
                    super::export::operation(format!(
                        "load preserved Lexicon snapshot {id}: {error}"
                    ))
                })?,
            };
            add_manifest_objects(&mut objects, &manifest)
                .map_err(|error| super::export::operation(format!("snapshot {id}: {error}")))?;
            self.add_topology_references(id, &manifest, &mut topology)?;
        }

        let snapshot_ids = snapshots
            .iter()
            .map(|snapshot| snapshot.id.clone())
            .collect::<BTreeSet<_>>();
        let delete_snapshots = snapshot_ids
            .difference(&preserved)
            .cloned()
            .collect::<Vec<_>>();
        let all_objects = list_objects(self)?.into_iter().collect::<BTreeSet<_>>();
        let delete_objects = all_objects
            .difference(&objects)
            .cloned()
            .collect::<Vec<_>>();

        let all_topology = self.list_topology_objects()?;
        let delete_topology_objects = all_topology.difference(&topology).cloned().collect();
        let delete_bootstrap_snapshots = self
            .list_bootstrap_snapshots()?
            .difference(&preserved)
            .cloned()
            .collect();
        Ok(GcPlan {
            preserved_topology_objects: topology.into_iter().collect(),
            delete_topology_objects,
            delete_bootstrap_snapshots,
            current_snapshot: current_id,
            preserved_snapshots: preserved.into_iter().collect(),
            delete_snapshots,
            preserved_objects: objects.into_iter().collect(),
            delete_objects,
        })
    }

    pub fn garbage_collect(
        &self,
        options: GcOptions,
        dry_run: bool,
    ) -> Result<GcResult, StorageError> {
        let _guard = self.lock()?;
        let plan = self.plan_gc(options)?;
        self.execute_gc_locked(plan, dry_run)
    }
}
