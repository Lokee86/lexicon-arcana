use crate::{FactHeader, FactRecord, FactStream, StorageError, ValidationError};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub header: FactHeader,
    pub records: Vec<FactRecord>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RecordGroups {
    pub(crate) owned: BTreeMap<String, Vec<FactRecord>>,
    pub(crate) shared: Vec<FactRecord>,
}

impl Analysis {
    pub fn new(header: FactHeader, records: Vec<FactRecord>) -> Self {
        Self { header, records }
    }

    pub fn parse(input: &str) -> Result<Self, StorageError> {
        let stream = FactStream::parse(input)?;
        Ok(Self {
            header: stream.header,
            records: stream.records,
        })
    }

    pub fn restrict_incremental_ownership(&mut self) {
        if !self.is_incremental() {
            return;
        }
        let allowed = self
            .header
            .changed_files
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|path| normalize_owner(path))
            .collect::<BTreeSet<_>>();
        let owners = node_owners(&self.records);
        self.records.retain(|record| {
            record_owner(record, &owners).is_none_or(|owner| allowed.contains(&owner))
        });
    }

    pub fn canonicalize(&mut self) -> Result<(), ValidationError> {
        crate::facts::sort_records(&mut self.records)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        crate::facts::validate_parts(&self.header, &self.records)
    }

    pub fn is_incremental(&self) -> bool {
        self.header.mode.as_deref() == Some("incremental")
    }

    pub(crate) fn groups(&self, allowed: Option<&BTreeSet<String>>) -> RecordGroups {
        let node_owners = node_owners(&self.records);
        let mut groups = RecordGroups::default();
        for record in &self.records {
            let owner = record_owner(record, &node_owners);
            if let Some(owner) = owner
                && allowed.is_none_or(|allowed| allowed.contains(&owner))
            {
                groups.owned.entry(owner).or_default().push(record.clone());
                continue;
            }
            groups.shared.push(record.clone());
        }
        groups
    }
}

pub(crate) fn normalized_paths(paths: &[String]) -> Vec<String> {
    let mut result = BTreeSet::new();
    for path in paths {
        if let Some(path) = normalize_owner(path) {
            result.insert(path);
        }
    }
    result.into_iter().collect()
}

fn node_owners(records: &[FactRecord]) -> BTreeMap<String, String> {
    records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => direct_owner(record).map(|owner| (node.id.clone(), owner)),
            _ => None,
        })
        .collect()
}

fn record_owner(record: &FactRecord, node_owners: &BTreeMap<String, String>) -> Option<String> {
    direct_owner(record).or_else(|| match record {
        FactRecord::Edge(edge) => node_owners.get(&edge.source).cloned(),
        FactRecord::Unresolved(value) => node_owners.get(&value.source).cloned(),
        FactRecord::Node(_) => None,
    })
}

fn direct_owner(record: &FactRecord) -> Option<String> {
    record
        .owner()
        .and_then(normalize_owner)
        .or_else(|| record.span().and_then(|span| normalize_owner(&span.path)))
        .or_else(|| match record {
            FactRecord::Node(node) if node.kind == "file" => normalize_owner(&node.path),
            _ => None,
        })
}

fn normalize_owner(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    if path.is_empty() || path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
        return None;
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            value => parts.push(value),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}
