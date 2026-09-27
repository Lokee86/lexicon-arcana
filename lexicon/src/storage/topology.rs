use std::collections::{BTreeMap, BTreeSet};

use crate::FactRecord;

use super::analysis::normalized_paths;
use super::{Analysis, StorageError, Store};

impl Store {
    pub fn requires_full_analysis(
        &self,
        language: &str,
        changed_files: &[String],
        analysis: &Analysis,
    ) -> Result<bool, StorageError> {
        if !analysis.is_incremental() {
            return Ok(true);
        }
        let (_, manifest) = self.current()?;
        let Some(entry) = manifest.language(language) else {
            return Ok(true);
        };
        let files: BTreeMap<&str, &str> = entry
            .files
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|file| (file.path.as_str(), file.object_id.as_str()))
            .collect();

        let selected: BTreeSet<String> = normalized_paths(changed_files).into_iter().collect();
        let mut added = BTreeSet::new();
        let mut previous = BTreeMap::new();
        for path in &selected {
            if let Some(object_id) = files.get(path.as_str()) {
                let object = self.load_object(object_id)?;
                previous.insert(path.clone(), relation_keys(&object.records)?);
            } else {
                added.insert(path.clone());
                previous.insert(path.clone(), BTreeSet::new());
            }
        }

        let groups = analysis.groups(None);
        for (owner, records) in groups.owned {
            if !records.iter().copied().any(is_relationship) {
                continue;
            }
            if !selected.contains(&owner) {
                return Ok(true);
            }
            let known = previous
                .get(&owner)
                .expect("selected owner has previous topology");
            for record in records
                .iter()
                .copied()
                .filter(|record| is_relationship(record))
            {
                let key = relation_key(record)?;
                if known.contains(&key) {
                    continue;
                }
                if let FactRecord::Unresolved(value) = record
                    && topology_sensitive_unresolved(&value.reason)
                {
                    return Ok(true);
                }
                if added.contains(&owner) {
                    continue;
                }
            }
        }
        Ok(false)
    }
}

fn topology_sensitive_unresolved(reason: &str) -> bool {
    matches!(reason, "ambiguous-target" | "generated-target")
}

fn relation_keys(records: &[FactRecord]) -> Result<BTreeSet<String>, StorageError> {
    records
        .iter()
        .filter(|record| is_relationship(record))
        .map(relation_key)
        .collect()
}

fn is_relationship(record: &FactRecord) -> bool {
    matches!(record, FactRecord::Edge(_) | FactRecord::Unresolved(_))
}

fn relation_key(record: &FactRecord) -> Result<String, StorageError> {
    let values = match record {
        FactRecord::Edge(edge) => vec![
            "edge",
            edge.source.as_str(),
            edge.target.as_str(),
            edge.relation.as_str(),
        ],
        FactRecord::Unresolved(value) => vec![
            "unresolved",
            value.source.as_str(),
            value.relation.as_str(),
            value.expression.as_str(),
            value.reason.as_str(),
            value.candidate_name.as_deref().unwrap_or(""),
        ],
        FactRecord::Node(_) => return Ok(String::new()),
    };
    serde_json::to_string(&values).map_err(StorageError::from)
}
