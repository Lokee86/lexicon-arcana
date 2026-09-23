use super::{SourceSpan, ValidationError};

pub(crate) fn repository_path(path: &str, allow_empty: bool) -> Result<(), ValidationError> {
    if path.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err(ValidationError::InvalidPath(path.to_owned()))
        };
    }
    if path.contains('\\')
        || path.starts_with('/')
        || path.ends_with('/')
        || path.as_bytes().get(1).is_some_and(|byte| *byte == b':')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(ValidationError::InvalidPath(path.to_owned()));
    }
    Ok(())
}

pub(crate) fn source_span(span: &SourceSpan) -> Result<(), ValidationError> {
    repository_path(&span.path, false)?;
    if span.start_line == 0
        || span.start_column == 0
        || span.end_line == 0
        || span.end_column == 0
        || (span.end_line, span.end_column) < (span.start_line, span.start_column)
    {
        return Err(ValidationError::InvalidSpan(span.path.clone()));
    }
    Ok(())
}

pub(crate) fn sorted_paths(paths: &[String]) -> Result<(), ValidationError> {
    for path in paths {
        repository_path(path, false)?;
    }
    if paths.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ValidationError::NonCanonicalOrder);
    }
    Ok(())
}
