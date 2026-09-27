use super::*;

impl RepositoryHeader {
    pub(super) fn validate(&self) -> Result<(), FormatError> {
        let mut cursor = u64::from(HEADER_LEN);
        for kind in SectionKind::ALL {
            let section = self.section(kind);
            let expected_offset = align_up(cursor)?;
            if section.offset % ALIGNMENT != 0 {
                return Err(FormatError::MisalignedSection(kind));
            }
            if section.offset != expected_offset {
                return Err(FormatError::SectionOutOfBounds(kind));
            }
            let end = section
                .offset
                .checked_add(section.byte_len)
                .ok_or(FormatError::SizeOverflow)?;
            if end > self.file_len {
                return Err(FormatError::SectionOutOfBounds(kind));
            }
            validate_section_shape(kind, section)?;
            cursor = end;
        }
        if align_up(cursor)? != self.file_len {
            return Err(FormatError::InvalidFileLength);
        }

        let node_count = self.section(SectionKind::Nodes).record_count;
        if node_count > u64::from(u32::MAX) {
            return Err(FormatError::TooManyNodes);
        }
        for kind in [
            SectionKind::NameIndex,
            SectionKind::PathIndex,
            SectionKind::KindIndex,
        ] {
            if self.section(kind).record_count != node_count {
                return Err(FormatError::IndexCountMismatch(kind));
            }
        }
        Ok(())
    }
}

fn validate_section_shape(
    kind: SectionKind,
    section: SectionDescriptor,
) -> Result<(), FormatError> {
    if kind == SectionKind::Strings {
        if section.record_count > u64::from(ABSENT_STRING_ID) {
            return Err(FormatError::TooManyStrings);
        }
        let minimum = section
            .record_count
            .checked_mul(STRING_INDEX_RECORD_LEN)
            .ok_or(FormatError::SizeOverflow)?;
        return (minimum <= section.byte_len)
            .then_some(())
            .ok_or(FormatError::InvalidSectionLength(kind));
    }

    if kind == SectionKind::Ownership {
        let fixed = section
            .record_count
            .checked_mul(FILE_OWNERSHIP_RECORD_LEN)
            .ok_or(FormatError::SizeOverflow)?;
        if fixed > section.byte_len || (section.byte_len - fixed) % CONTRIBUTION_RECORD_LEN != 0 {
            return Err(FormatError::InvalidSectionLength(kind));
        }
        return Ok(());
    }

    let record_len = match kind {
        SectionKind::Nodes => NODE_RECORD_LEN,
        SectionKind::Edges => EDGE_RECORD_LEN,
        SectionKind::Unresolved => UNRESOLVED_RECORD_LEN,
        SectionKind::NameIndex => NAME_INDEX_RECORD_LEN,
        SectionKind::PathIndex => PATH_INDEX_RECORD_LEN,
        SectionKind::KindIndex => KIND_INDEX_RECORD_LEN,
        SectionKind::Strings | SectionKind::Ownership => unreachable!(),
    };
    let expected = section
        .record_count
        .checked_mul(record_len)
        .ok_or(FormatError::SizeOverflow)?;
    (expected == section.byte_len)
        .then_some(())
        .ok_or(FormatError::InvalidSectionLength(kind))
}

pub(super) fn align_up(value: u64) -> Result<u64, FormatError> {
    value
        .checked_add(ALIGNMENT - 1)
        .map(|value| value & !(ALIGNMENT - 1))
        .ok_or(FormatError::SizeOverflow)
}
