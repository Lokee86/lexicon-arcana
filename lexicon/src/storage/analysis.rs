use crate::{FactHeader, FactRecord, FactStream, StorageError, ValidationError};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub header: FactHeader,
    pub records: Vec<FactRecord>,
}

#[derive(Debug, Default)]
pub(crate) struct RecordGroups<'a> {
    pub(crate) owned: BTreeMap<String, Vec<&'a FactRecord>>,
    pub(crate) shared: Vec<&'a FactRecord>,
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
        let retained = self
            .records
            .iter()
            .map(|record| {
                record_owner(record, &owners)
                    .as_deref()
                    .is_none_or(|owner| allowed.contains(owner))
            })
            .collect::<Vec<_>>();
        drop(owners);
        let mut index = 0;
        self.records.retain(|_| {
            let keep = retained[index];
            index += 1;
            keep
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

    pub(crate) fn groups(&self, allowed: Option<&BTreeSet<String>>) -> RecordGroups<'_> {
        let started = crate::perf::start();
        let node_owners = node_owners(&self.records);
        let mut groups = RecordGroups::default();
        for record in &self.records {
            let owner = record_owner(record, &node_owners);
            if let Some(owner) = owner
                && allowed.is_none_or(|allowed| allowed.contains(owner.as_ref()))
            {
                if let Some(records) = groups.owned.get_mut(owner.as_ref()) {
                    records.push(record);
                } else {
                    groups.owned.insert(owner.into_owned(), vec![record]);
                }
                continue;
            }
            groups.shared.push(record);
        }
        if let Some(started) = started {
            let owned_records = groups
                .owned
                .values()
                .map(|records| records.len() as u64)
                .sum::<u64>();
            crate::perf::emit(
                &format!("{}.ownership_partitioning", self.header.language),
                started.elapsed(),
                &[
                    ("fact_count", self.records.len() as u64),
                    ("ownership_partitions", groups.owned.len() as u64),
                    ("owned_records", owned_records),
                    ("shared_records", groups.shared.len() as u64),
                    ("record_clones", 0),
                    ("record_references", self.records.len() as u64),
                ],
            );
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

fn node_owners<'a>(records: &'a [FactRecord]) -> BTreeMap<&'a str, Cow<'a, str>> {
    records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => direct_owner(record).map(|owner| (node.id.as_str(), owner)),
            _ => None,
        })
        .collect()
}

fn record_owner<'a>(
    record: &'a FactRecord,
    node_owners: &BTreeMap<&'a str, Cow<'a, str>>,
) -> Option<Cow<'a, str>> {
    direct_owner(record).or_else(|| match record {
        FactRecord::Edge(edge) => node_owners.get(edge.source.as_str()).cloned(),
        FactRecord::Unresolved(value) => node_owners.get(value.source.as_str()).cloned(),
        FactRecord::Node(_) => None,
    })
}

fn direct_owner(record: &FactRecord) -> Option<Cow<'_, str>> {
    record
        .owner()
        .and_then(normalize_owner_cow)
        .or_else(|| {
            record
                .span()
                .and_then(|span| normalize_owner_cow(&span.path))
        })
        .or_else(|| match record {
            FactRecord::Node(node) if node.kind == "file" => normalize_owner_cow(&node.path),
            _ => None,
        })
}

fn normalize_owner(path: &str) -> Option<String> {
    normalize_owner_cow(path).map(Cow::into_owned)
}

fn normalize_owner_cow(path: &str) -> Option<Cow<'_, str>> {
    if path.is_empty() || path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
        return None;
    }

    let mut needs_normalization = path.contains('\\');
    for part in path.split(['/', '\\']) {
        match part {
            ".." => return None,
            "" | "." => needs_normalization = true,
            _ => {}
        }
    }
    if !needs_normalization {
        return Some(Cow::Borrowed(path));
    }

    let parts = path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| Cow::Owned(parts.join("/")))
}
