mod incremental;
mod jsonl;
mod model;
mod order;
mod path;
mod validate;

pub use jsonl::FactStream;
pub(crate) use jsonl::{record_from_value, sort_records};
pub use model::{EdgeRecord, FactHeader, FactRecord, NodeRecord, SourceSpan, UnresolvedRecord};
pub use validate::ValidationError;

pub(crate) fn validate_parts(
    header: &FactHeader,
    records: &[FactRecord],
) -> Result<(), ValidationError> {
    validate::parts(header, records)
}
