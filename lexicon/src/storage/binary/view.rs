use crate::{FactObject, FactRecord};

#[derive(Clone, Copy)]
pub(crate) enum RecordSelection<'a> {
    Direct(&'a [FactRecord]),
    Refs(&'a [&'a FactRecord]),
}

impl RecordSelection<'_> {
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Direct(records) => records.len(),
            Self::Refs(records) => records.len(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn get(&self, index: usize) -> &FactRecord {
        match self {
            Self::Direct(records) => &records[index],
            Self::Refs(records) => records[index],
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &FactRecord> {
        (0..self.len()).map(|index| self.get(index))
    }
}

pub(crate) struct ObjectView<'a> {
    pub(crate) language: &'a str,
    pub(crate) owner: &'a str,
    pub(crate) source_content_id: &'a str,
    pub(crate) adapter_version: &'a str,
    pub(crate) schema_version: u64,
    pub(crate) analysis_config_id: &'a str,
    pub(crate) records: RecordSelection<'a>,
}

impl<'a> From<&'a FactObject> for ObjectView<'a> {
    fn from(object: &'a FactObject) -> Self {
        Self {
            language: &object.language,
            owner: &object.owner,
            source_content_id: &object.source_content_id,
            adapter_version: &object.adapter_version,
            schema_version: object.schema_version,
            analysis_config_id: &object.analysis_config_id,
            records: RecordSelection::Direct(&object.records),
        }
    }
}
