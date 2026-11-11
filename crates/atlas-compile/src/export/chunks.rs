use std::collections::BTreeMap;

use atlas_boundary::publication::VerifiedPart;
use serde::Serialize;

use crate::export::{Artifacts, ExportError, MAX_FILE_BYTES, insert_bytes};

#[derive(Clone, Copy)]
pub(crate) enum ChunkKind {
    Array,
    GeoJson,
}

pub(crate) fn insert_chunks<T: Serialize>(
    output: &mut Artifacts,
    prefix: &str,
    values: &[T],
    kind: ChunkKind,
) -> Result<Vec<String>, ExportError> {
    partition(output, prefix, values, kind, MAX_FILE_BYTES)
}

pub(crate) fn insert_verified_array<T: Serialize>(
    output: &mut Artifacts,
    prefix: &str,
    values: &[T],
) -> Result<Vec<VerifiedPart>, ExportError> {
    if values.is_empty() {
        let path = format!("{prefix}/000.json");
        let bytes = b"[]".to_vec();
        let dir_prefix = format!("{prefix}/");
        let part = VerifiedPart::from_bytes(path.clone(), &dir_prefix, &bytes)?;
        insert_bytes(output, &path, bytes)?;
        return Ok(vec![part]);
    }
    let paths = partition(output, prefix, values, ChunkKind::Array, MAX_FILE_BYTES)?;
    let dir_prefix = format!("{prefix}/");
    paths
        .into_iter()
        .map(|path| {
            let bytes = output
                .get(std::path::Path::new(&path))
                .map_or(&[][..], Vec::as_slice);
            Ok(VerifiedPart::from_bytes(path, &dir_prefix, bytes)?)
        })
        .collect()
}

pub(crate) fn insert_verified_map<K: Serialize, V: Serialize>(
    output: &mut Artifacts,
    prefix: &str,
    entries: &BTreeMap<K, V>,
) -> Result<Vec<VerifiedPart>, ExportError> {
    let dir_prefix = format!("{prefix}/");
    if entries.is_empty() {
        let path = format!("{prefix}/000.json");
        let bytes = b"{}".to_vec();
        let part = VerifiedPart::from_bytes(path.clone(), &dir_prefix, &bytes)?;
        insert_bytes(output, &path, bytes)?;
        return Ok(vec![part]);
    }
    let mut parts = Vec::new();
    let mut bytes = vec![b'{'];
    for (key, value) in entries {
        let mut pair = serde_json::to_vec(key)?;
        pair.push(b':');
        pair.extend_from_slice(&serde_json::to_vec(value)?);
        let minimum = 2usize.saturating_add(pair.len());
        if minimum >= MAX_FILE_BYTES {
            return Err(ExportError::Oversize {
                path: prefix.into(),
                bytes: minimum,
            });
        }
        if bytes.len() > 1 && bytes.len() + 2 + pair.len() >= MAX_FILE_BYTES {
            bytes.push(b'}');
            let path = format!("{prefix}/{:03}.json", parts.len());
            let part = VerifiedPart::from_bytes(path.clone(), &dir_prefix, &bytes)?;
            insert_bytes(output, &path, std::mem::replace(&mut bytes, vec![b'{']))?;
            parts.push(part);
        }
        if bytes.len() > 1 {
            bytes.push(b',');
        }
        bytes.extend_from_slice(&pair);
    }
    bytes.push(b'}');
    let path = format!("{prefix}/{:03}.json", parts.len());
    let part = VerifiedPart::from_bytes(path.clone(), &dir_prefix, &bytes)?;
    insert_bytes(output, &path, bytes)?;
    parts.push(part);
    Ok(parts)
}

pub(crate) fn insert_verified_parts(
    output: &mut Artifacts,
    prefix: &str,
    value: &impl Serialize,
) -> Result<Vec<VerifiedPart>, ExportError> {
    let encoded = serde_json::to_vec(value)?;
    let dir_prefix = format!("{prefix}/");
    let mut parts = Vec::new();
    for (index, slice) in encoded.chunks(2_000_000).enumerate() {
        let path = format!("{prefix}/{index:03}.part");
        let part = VerifiedPart::from_bytes(path.clone(), &dir_prefix, slice)?;
        insert_bytes(output, &path, slice.to_vec())?;
        parts.push(part);
    }
    Ok(parts)
}

fn partition<T: Serialize>(
    output: &mut Artifacts,
    prefix: &str,
    values: &[T],
    kind: ChunkKind,
    limit: usize,
) -> Result<Vec<String>, ExportError> {
    let (header, tail, extension): (&[u8], &[u8], &str) = match kind {
        ChunkKind::Array => (b"[", b"]", "json"),
        ChunkKind::GeoJson => (
            b"{\"type\":\"FeatureCollection\",\"features\":[",
            b"]}",
            "geojson",
        ),
    };
    let mut paths = Vec::new();
    let mut bytes = header.to_vec();
    for value in values {
        let encoded = serde_json::to_vec(value)?;
        let minimum = header.len() + encoded.len() + tail.len();
        if minimum >= limit {
            return Err(ExportError::Oversize {
                path: prefix.into(),
                bytes: minimum,
            });
        }
        if bytes.len() > header.len() && bytes.len() + 1 + encoded.len() + tail.len() >= limit {
            bytes.extend_from_slice(tail);
            let path = format!("{prefix}/{:03}.{extension}", paths.len());
            insert_bytes(
                output,
                &path,
                std::mem::replace(&mut bytes, header.to_vec()),
            )?;
            paths.push(path);
        }
        if bytes.len() > header.len() {
            bytes.push(b',');
        }
        bytes.extend_from_slice(&encoded);
    }
    if bytes.len() > header.len() {
        bytes.extend_from_slice(tail);
        let path = format!("{prefix}/{:03}.{extension}", paths.len());
        insert_bytes(output, &path, bytes)?;
        paths.push(path);
    }
    Ok(paths)
}

#[cfg(test)]
mod tests;
