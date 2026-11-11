use std::collections::{BTreeMap, BTreeSet};

use atlas_boundary::publication::{
    CatalogItem, CatalogShardSummary, ShardSummary, ShardTemporalSummary, VerifiedPart,
};
use atlas_core::identity::EntityId;
use atlas_geo::body::BodyId;

use super::{Artifacts, ExportError, IndexedEntity, MAX_FILE_BYTES, insert_bytes};

pub(crate) mod summary;

pub(crate) const DEFAULT_TARGET_SHARD_BYTES: usize = 500_000;
const MAX_QUAD_ZOOM: u8 = 12;

struct PreparedEntity {
    id: EntityId,
    hex: String,
    bounds: Option<[f64; 4]>,
    temporal: ShardTemporalSummary,
    catalog: CatalogItem,
    encoded_pair: Vec<u8>,
}

struct PreparedGroup {
    hex: String,
    overview: bool,
    body: BodyId,
    bounds: Option<[f64; 4]>,
    members: Vec<PreparedEntity>,
}

type TierBuckets =
    BTreeMap<(&'static str, bool, usize), (Option<BodyId>, Vec<PreparedGroup>, Vec<PreparedGroup>)>;

fn prepare_groups(
    entities: &BTreeMap<EntityId, IndexedEntity<'_>>,
    geometry_spans: &summary::GeometrySpans,
    cross_cited: &BTreeSet<EntityId>,
    reps: &BTreeMap<EntityId, EntityId>,
) -> Result<BTreeMap<EntityId, PreparedGroup>, ExportError> {
    let mut grouped: BTreeMap<EntityId, PreparedGroup> = BTreeMap::new();
    for (&id, entry) in entities {
        let overview = entry.entity.kind.is_overview()
            || entry.geometry.is_some()
            || cross_cited.contains(&id);
        let bounds = summary::entity_bounds(entry);
        let temporal = summary::entity_temporal(entry, geometry_spans.get(&id));
        let catalog = summary::build_catalog_item(entry);
        let mut encoded_pair = serde_json::to_vec(&id)?;
        encoded_pair.push(b':');
        encoded_pair.extend_from_slice(&serde_json::to_vec(entry)?);
        let hex = id.0.simple().to_string();
        let rep = reps.get(&id).copied().unwrap_or(id);
        let group = grouped.entry(rep).or_insert_with(|| PreparedGroup {
            hex: hex.clone(),
            overview,
            body: entry.entity.body,
            bounds: None,
            members: Vec::new(),
        });
        group.overview |= overview;
        group.bounds = summary::merge_bounds(group.bounds, bounds);
        group.members.push(PreparedEntity {
            id,
            hex,
            bounds,
            temporal,
            catalog,
            encoded_pair,
        });
    }
    for (rep, group) in &grouped {
        let bytes = object_bytes(std::slice::from_ref(group));
        if bytes >= MAX_FILE_BYTES {
            return Err(ExportError::Oversize {
                path: format!("entities/index/{}", rep.0),
                bytes,
            });
        }
    }
    Ok(grouped)
}

pub(crate) fn publish(
    output: &mut Artifacts,
    entities: &BTreeMap<EntityId, IndexedEntity<'_>>,
    geometry_spans: &summary::GeometrySpans,
    cross_cited: &BTreeSet<EntityId>,
    target_bytes: usize,
) -> Result<(Vec<ShardSummary>, Vec<CatalogShardSummary>), ExportError> {
    let split_limit = target_bytes.clamp(512, MAX_FILE_BYTES.saturating_sub(1));
    let (reps, ranks) = summary::citation_ranks(entities);
    let grouped = prepare_groups(entities, geometry_spans, cross_cited, &reps)?;
    let mut groups: TierBuckets = BTreeMap::new();
    for (rep, group) in grouped {
        let slug = summary::body_slug(group.body);
        let rank = if group.overview {
            ranks.get(&rep).copied().unwrap_or(0)
        } else {
            0
        };
        let bucket = groups.entry((slug, group.overview, rank)).or_default();
        bucket.0 = Some(group.body);
        if group.bounds.is_some() {
            bucket.1.push(group);
        } else {
            bucket.2.push(group);
        }
    }
    let mut shards = Vec::new();
    let mut catalog_items = Vec::new();
    for ((slug, overview, rank), (body_opt, located, unlocated)) in groups {
        let Some(body) = body_opt else {
            continue;
        };
        let base = match (overview, rank) {
            (true, 0) => format!("{slug}/overview"),
            (true, r) => format!("{slug}/overview/r{r}"),
            (false, _) => format!("{slug}/detail"),
        };
        let mut cells = Vec::new();
        split_quadtree(
            &base,
            (0, 0, 0),
            [-180.0, -90.0, 180.0, 90.0],
            located,
            split_limit,
            &mut cells,
        );
        if !unlocated.is_empty() {
            cells.push((format!("{base}/unlocated"), unlocated));
        }
        for (cell_id, items) in cells {
            let mut sink = ShardSink {
                output,
                shards: &mut shards,
                catalog_items: &mut catalog_items,
            };
            emit_prefix_shards(
                &mut sink,
                &cell_id,
                (body, overview),
                items,
                (0, split_limit),
            )?;
        }
    }
    shards.sort_by(|a, b| a.id.cmp(&b.id));
    summary::assign_dependencies(entities, &catalog_items, &mut shards);
    catalog_items.sort_by_key(|item| item.id);
    let catalog_shards = emit_catalog_shards(output, catalog_items, split_limit)?;
    Ok((shards, catalog_shards))
}

fn object_bytes(groups: &[PreparedGroup]) -> usize {
    let count: usize = groups.iter().map(|g| g.members.len()).sum();
    if count == 0 {
        return 2;
    }
    let payload: usize = groups
        .iter()
        .flat_map(|g| &g.members)
        .map(|item| item.encoded_pair.len())
        .sum();
    2usize
        .saturating_add(payload)
        .saturating_add(count.saturating_sub(1))
}

fn split_quadtree(
    base: &str,
    (z, x, y): (u8, u32, u32),
    [cw, cs, ce, cn]: [f64; 4],
    items: Vec<PreparedGroup>,
    limit: usize,
    out: &mut Vec<(String, Vec<PreparedGroup>)>,
) {
    if items.is_empty() {
        return;
    }
    let cell_id = format!("{base}/z{z}_x{x}_y{y}");
    if z >= MAX_QUAD_ZOOM || items.len() <= 1 || object_bytes(&items) <= limit {
        out.push((cell_id, items));
        return;
    }
    let mid_lon = (cw + ce) * 0.5;
    let mid_lat = (cs + cn) * 0.5;
    let quadrants: [([f64; 4], (u32, u32)); 4] = [
        ([cw, cs, mid_lon, mid_lat], (x * 2, y * 2)),
        ([mid_lon, cs, ce, mid_lat], (x * 2 + 1, y * 2)),
        ([cw, mid_lat, mid_lon, cn], (x * 2, y * 2 + 1)),
        ([mid_lon, mid_lat, ce, cn], (x * 2 + 1, y * 2 + 1)),
    ];
    let mut retained = Vec::new();
    let mut children: [Vec<PreparedGroup>; 4] = Default::default();
    let mut moved = false;
    for item in items {
        let placed = item.bounds.and_then(|[w, s, e, n]| {
            quadrants
                .iter()
                .position(|([qw, qs, qe, qn], _)| w >= *qw && e <= *qe && s >= *qs && n <= *qn)
        });
        if let Some(idx) = placed
            && let Some(bucket) = children.get_mut(idx)
        {
            bucket.push(item);
            moved = true;
        } else {
            retained.push(item);
        }
    }
    if !moved {
        out.push((cell_id, retained));
        return;
    }
    if !retained.is_empty() {
        out.push((cell_id, retained));
    }
    let next_z = z.wrapping_add(1);
    for (child_items, (rect, (cx, cy))) in children.into_iter().zip(quadrants) {
        split_quadtree(base, (next_z, cx, cy), rect, child_items, limit, out);
    }
}

struct ShardSink<'a> {
    output: &'a mut Artifacts,
    shards: &'a mut Vec<ShardSummary>,
    catalog_items: &'a mut Vec<CatalogItem>,
}

fn emit_prefix_shards(
    sink: &mut ShardSink<'_>,
    cell_id: &str,
    (body, overview): (BodyId, bool),
    groups: Vec<PreparedGroup>,
    (depth, limit): (usize, usize),
) -> Result<(), ExportError> {
    if groups.is_empty() {
        return Ok(());
    }
    let expected_bytes = object_bytes(&groups);
    if groups.len() > 1 && depth < 32 && expected_bytes > limit {
        let next_depth = depth.wrapping_add(1);
        let mut buckets: BTreeMap<String, Vec<PreparedGroup>> = BTreeMap::new();
        for group in groups {
            let prefix = group.hex.get(..next_depth).unwrap_or(&group.hex).to_owned();
            buckets.entry(prefix).or_default().push(group);
        }
        for (_, sub) in buckets {
            emit_prefix_shards(sink, cell_id, (body, overview), sub, (next_depth, limit))?;
        }
        return Ok(());
    }
    let mut items: Vec<PreparedEntity> = groups.into_iter().flat_map(|g| g.members).collect();
    items.sort_by_key(|item| item.id);
    let shard_id = if depth == 0 {
        cell_id.to_owned()
    } else {
        let prefix = items
            .first()
            .and_then(|first| first.hex.get(..depth))
            .unwrap_or("0");
        format!("{cell_id}/{prefix}")
    };
    let mut bytes = Vec::with_capacity(expected_bytes);
    bytes.push(b'{');
    let mut bounds = None;
    let mut temporal = ShardTemporalSummary::default();
    let entity_count = items.len();
    for (idx, mut item) in items.into_iter().enumerate() {
        if idx > 0 {
            bytes.push(b',');
        }
        bytes.extend_from_slice(&item.encoded_pair);
        bounds = summary::merge_bounds(bounds, item.bounds);
        temporal = summary::merge_temporal(temporal, &item.temporal);
        item.catalog.shard = shard_id.clone();
        sink.catalog_items.push(item.catalog);
    }
    bytes.push(b'}');
    let path = format!("entities/index/{shard_id}.json");
    let part = VerifiedPart::from_bytes(path.clone(), "entities/index/", &bytes)?;
    insert_bytes(sink.output, &path, bytes)?;
    let summary = ShardSummary {
        id: shard_id,
        part,
        body,
        overview,
        bounds,
        temporal,
        depends_on: Vec::new(),
        entity_count,
    };
    summary.validate()?;
    sink.shards.push(summary);
    Ok(())
}

fn emit_catalog_shards(
    output: &mut Artifacts,
    items: Vec<CatalogItem>,
    limit: usize,
) -> Result<Vec<CatalogShardSummary>, ExportError> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let encoded: Vec<(String, Vec<u8>)> = items
        .into_iter()
        .map(|item| Ok((item.id.0.simple().to_string(), serde_json::to_vec(&item)?)))
        .collect::<Result<_, serde_json::Error>>()?;
    let mut summaries = Vec::new();
    split_catalog(output, encoded, 0, limit, &mut summaries)?;
    summaries.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    Ok(summaries)
}

fn split_catalog(
    output: &mut Artifacts,
    items: Vec<(String, Vec<u8>)>,
    depth: usize,
    limit: usize,
    out: &mut Vec<CatalogShardSummary>,
) -> Result<(), ExportError> {
    if items.is_empty() {
        return Ok(());
    }
    let size = 2usize
        .saturating_add(items.iter().map(|(_, b)| b.len()).sum::<usize>())
        .saturating_add(items.len().saturating_sub(1));
    if items.len() > 1 && depth < 32 && size > limit {
        let next_depth = depth.wrapping_add(1);
        let mut buckets: BTreeMap<String, Vec<(String, Vec<u8>)>> = BTreeMap::new();
        for (hex, bytes) in items {
            let prefix = hex.get(..next_depth).unwrap_or(&hex).to_owned();
            buckets.entry(prefix).or_default().push((hex, bytes));
        }
        for (_, sub) in buckets {
            split_catalog(output, sub, next_depth, limit, out)?;
        }
        return Ok(());
    }
    let prefix = if depth == 0 {
        "all".to_owned()
    } else {
        items
            .first()
            .and_then(|(hex, _)| hex.get(..depth))
            .unwrap_or("0")
            .to_owned()
    };
    let count = items.len();
    let mut bytes = Vec::with_capacity(size);
    bytes.push(b'[');
    for (idx, (_, item)) in items.into_iter().enumerate() {
        if idx > 0 {
            bytes.push(b',');
        }
        bytes.extend_from_slice(&item);
    }
    bytes.push(b']');
    let path = format!("entities/catalog/{prefix}.json");
    let part = VerifiedPart::from_bytes(path.clone(), "entities/catalog/", &bytes)?;
    insert_bytes(output, &path, bytes)?;
    out.push(CatalogShardSummary {
        prefix,
        part,
        count,
    });
    Ok(())
}
