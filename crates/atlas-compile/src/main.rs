//! Command-line validation and compilation of Historical Atlas datasets.

use std::{error::Error, path::Path};

use atlas_compile::{
    export::{compile, write_blocking},
    input::load_blocking,
    reconstruction::reconstruct_blocking,
    registration::register_blocking,
    slice::write_slice_blocking,
};
use tracing::info;

fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, root] if command == "verify" => {
            let input = load_blocking(Path::new(root))?;
            let artifacts = compile(&input)?;
            info!(
                sources = input.catalog.sources.len(),
                entities = input.data.entities.len(),
                records = input.data.records.len(),
                artifacts = artifacts.len(),
                "Ledger verified"
            );
        }
        [command, root, destination] if command == "build" => {
            let input = load_blocking(Path::new(root))?;
            let artifacts = compile(&input)?;
            write_blocking(Path::new(destination), &artifacts)?;
            info!(
                files = artifacts.len(),
                destination, "Static indexes compiled"
            );
        }
        [command, root, request, destination] if command == "register" => {
            let result =
                register_blocking(Path::new(root), Path::new(request), Path::new(destination))?;
            info!(
                controls = result.control_quality.points,
                rms = result.control_quality.rms,
                units = result.job.target_frame.units,
                destination,
                "Sheet registered"
            );
        }
        [command, root, request, destination] if command == "reconstruct" => {
            let files =
                reconstruct_blocking(Path::new(root), Path::new(request), Path::new(destination))?;
            info!(files, destination, "Plate reconstruction compiled");
        }
        [command, root, ticks, selection, destination] if command == "slice" => {
            let files =
                write_slice_blocking(Path::new(root), ticks, selection, Path::new(destination))?;
            info!(files, ticks, selection, destination, "Time slice compiled");
        }
        _ => {
            return Err(
                "usage: atlas-compile verify <data> | build <data> <new-output-directory> | register <data> <controls.json> <new-result.json> | slice <data> <tt-ms> <nominal|possible|certain> <new-output-directory> | reconstruct <data> <request.json> <new-output-directory>".into(),
            );
        }
    }
    Ok(())
}
