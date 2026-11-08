//! Native Historical Atlas entry point.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let mut arguments = std::env::args_os().skip(1);
    let directory = arguments
        .next()
        .ok_or("usage: atlas-app <publication-directory>")?;
    if arguments.next().is_some() {
        return Err("usage: atlas-app <publication-directory>".into());
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
    let result = atlas_app::host::run_blocking(atlas);
    drop(entered);
    runtime.shutdown_timeout(std::time::Duration::from_secs(5));
    Ok(result?)
}

#[cfg(target_arch = "wasm32")]
fn main() {}
