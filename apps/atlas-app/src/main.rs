//! Native Historical Atlas entry point.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let mut arguments = std::env::args_os().skip(1);
    let directory = arguments
        .next()
        .ok_or("usage: atlas-app <publication-directory> [--overlay <package.json>]")?;
    let overlay = match arguments.next() {
        None => None,
        Some(flag) if flag == "--overlay" => {
            let path = arguments.next().ok_or("--overlay requires a JSON file")?;
            let path = std::path::Path::new(&path);
            Some(load_overlay_blocking(path))
        }
        Some(_) => return Err("expected --overlay <package.json>".into()),
    };
    if arguments.next().is_some() {
        return Err("usage: atlas-app <publication-directory> [--overlay <package.json>]".into());
    }
    let atlas = atlas_app::publication::load_blocking(std::path::Path::new(&directory))?;
    tracing::info!(
        entities = atlas.data.entities.len(),
        records = atlas.data.records.len(),
        "loaded atlas"
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("atlas-map")
        .build()?;
    let entered = runtime.enter();
    let result = atlas_app::host::run_blocking(atlas, overlay);
    drop(entered);
    runtime.shutdown_timeout(std::time::Duration::from_secs(5));
    Ok(result?)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_overlay_blocking(path: &std::path::Path) -> atlas_app::overlay::OverlayInput {
    atlas_app::overlay::OverlayInput {
        origin: path.display().to_string(),
        content: std::fs::read_to_string(path).map_err(|error| error.to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
