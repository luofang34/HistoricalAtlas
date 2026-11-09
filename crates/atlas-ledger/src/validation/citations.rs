use std::collections::BTreeMap;

use super::{ValidationError, fail};
use crate::model::{Record, Source, SourceCatalog};

pub(super) fn validate_sources(
    catalog: &SourceCatalog,
) -> Result<BTreeMap<&str, &Source>, ValidationError> {
    let mut sources = BTreeMap::new();
    for source in &catalog.sources {
        if source.id.trim().is_empty() || source.title.trim().is_empty() {
            return Err(fail(&source.id, "source requires an id and title"));
        }
        match (source.scan_pages, &source.scan_sha256) {
            (Some(pages), Some(digest))
                if pages > 0
                    && digest.len() == 64
                    && digest.bytes().all(|b| b.is_ascii_hexdigit()) => {}
            (None, None)
                if source.url.is_some()
                    || source.capture.as_ref().is_some_and(|capture| {
                        capture.archive.is_some() || capture.sha256.is_some()
                    }) => {}
            _ => {
                return Err(fail(
                    &source.id,
                    "source requires an archive locator, a URL, or paired scan page count and SHA-256 digest",
                ));
            }
        }
        if let Some(capture) = &source.capture
            && (capture.archive.as_ref().is_some_and(|archive| {
                archive.repository.trim().is_empty() || archive.accession.trim().is_empty()
            }) || capture
                .sortie
                .iter()
                .chain(&capture.frame)
                .any(|value| value.trim().is_empty()))
        {
            return Err(fail(&source.id, "capture identifiers must be nonempty"));
        }
        if source
            .capture
            .as_ref()
            .and_then(|capture| capture.sha256.as_ref())
            .is_some_and(|digest| {
                digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            return Err(fail(
                &source.id,
                "capture fingerprint must be a SHA-256 digest",
            ));
        }
        if source.url.as_deref().is_some_and(|url| !valid_url(url)) {
            return Err(fail(&source.id, "source URL must use HTTP(S) with a host"));
        }
        if sources.insert(source.id.as_str(), source).is_some() {
            return Err(fail(&source.id, "duplicate source id"));
        }
    }
    for source in sources.values() {
        if let Some(date) = source
            .capture
            .as_ref()
            .and_then(|capture| capture.date.as_ref())
        {
            super::dates::validate_date(&source.id, date, &sources)?;
        }
    }
    Ok(sources)
}

fn valid_url(url: &str) -> bool {
    let Some(remainder) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let host = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    !host.is_empty()
        && !host.contains('@')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

pub(super) fn validate_citation(record: &Record, source: &Source) -> Result<(), ValidationError> {
    let page_valid = match (source.scan_pages, record.scan_page) {
        (Some(total), Some(page)) => (1..=total).contains(&page),
        (None, None) => true,
        _ => false,
    };
    if !page_valid
        || record
            .page_or_sheet
            .as_ref()
            .is_some_and(|page| page.trim().is_empty())
        || (source.scan_pages.is_some() && record.page_or_sheet.is_none())
        || record.locator.trim().is_empty()
        || !record.observation.has_content()
    {
        return Err(fail(
            &record.key,
            "invalid page, locator, or empty observation",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
