use std::collections::BTreeMap;

use atlas_core::calendar::SourceDate;

use super::{ValidationError, caused, fail};
use crate::model::{Record, Source};

pub(super) fn validate_dates(
    record: &Record,
    sources: &BTreeMap<&str, &Source>,
) -> Result<(), ValidationError> {
    for date in record.source_date.iter().chain(
        record
            .observation
            .former_names
            .iter()
            .filter_map(|name| name.source_date.as_ref()),
    ) {
        validate_date(&record.key, date, sources)?;
    }
    Ok(())
}

fn validate_date(
    key: &str,
    date: &SourceDate,
    sources: &BTreeMap<&str, &Source>,
) -> Result<(), ValidationError> {
    if date.text.trim().is_empty() || date.convention.trim().is_empty() {
        return Err(fail(
            key,
            "source date requires original text and convention",
        ));
    }
    if let Some(conversion) = &date.era_conversion {
        conversion.to_estimate(0).map_err(|e| caused(key, e))?;
        if !sources
            .get(conversion.source_id.as_str())
            .is_some_and(|source| source.url.is_some())
        {
            return Err(fail(
                key,
                format!("unknown online calendar source {}", conversion.source_id),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
