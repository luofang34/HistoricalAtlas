//! Body-separated MVT source observations with absolute TileJSON endpoints.

use std::{collections::BTreeMap, path::Path};

use atlas_geo::body::BodyId;
use serde_json::json;

use crate::{
    export::{Artifacts, compile, insert, insert_bytes, write_blocking},
    input::{LedgerInput, load_blocking},
};

mod encoding;
mod error;
mod features;
mod metadata;
mod projection;

pub use error::TileError;

/// Compile checked coordinate records, evidence indexes, and body-specific styles.
///
/// `base_url` names the output directory at its final HTTP(S) location. Sparse
/// tile inventories accompany each tileset. Zoom levels range from zero through
/// `maxzoom`, inclusive, with buffered points for continuity across tile edges.
pub fn compile_tiles(
    input: &LedgerInput,
    base_url: &str,
    maxzoom: u8,
) -> Result<Artifacts, TileError> {
    let base = metadata::publication_base(base_url)?;
    if maxzoom > 30 {
        return Err(TileError::Zoom(maxzoom));
    }
    let mut output = compile(input)?;
    let mut bodies = Vec::new();
    for (body, name) in [
        (BodyId::Earth, "earth"),
        (BodyId::Moon, "moon"),
        (BodyId::Mars, "mars"),
    ] {
        let points = features::collect(input, body, &base)?;
        if points.is_empty() {
            continue;
        }
        let mut tiles = BTreeMap::<_, Vec<_>>::new();
        for point in &points {
            for zoom in 0..=maxzoom {
                for (address, position) in projection::placements(point.projected, zoom) {
                    tiles.entry(address).or_default().push((point, position));
                }
            }
        }
        let inventory: Vec<_> = tiles.keys().copied().collect();
        for (address, points) in tiles {
            let path = format!(
                "tiles/{name}/observations/{}/{}/{}.mvt",
                address.z, address.x, address.y
            );
            insert_bytes(&mut output, &path, encoding::encode(&points))?;
        }
        insert(
            &mut output,
            &format!("tiles/{name}/observations/index.json"),
            &inventory,
        )?;
        metadata::insert_body(&mut output, &base, name, maxzoom, &points)?;
        bodies.push(json!({
            "body": name,
            "tilejson": format!("{base}tiles/{name}/observations.json"),
            "style": format!("{base}styles/{name}.json"),
            "records": points.len()
        }));
    }
    insert(&mut output, "tiles/index.json", &bodies)?;
    Ok(output)
}

/// Load a ledger and write its map artifacts into a new publication directory.
pub fn write_tiles_blocking(
    root: &Path,
    base_url: &str,
    maxzoom: u8,
    destination: &Path,
) -> Result<usize, TileError> {
    let input = load_blocking(root)?;
    let artifacts = compile_tiles(&input, base_url, maxzoom)?;
    write_blocking(destination, &artifacts)?;
    Ok(artifacts.len())
}

#[cfg(test)]
mod tests;
