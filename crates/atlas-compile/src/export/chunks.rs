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
