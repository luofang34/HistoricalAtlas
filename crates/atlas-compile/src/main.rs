//! Command-line validation and compilation of Historical Atlas datasets.

use std::{error::Error, path::Path};

use atlas_compile::{
    boundary::boundary_blocking,
    derivation::derive_blocking,
    export::{compile, write_blocking},
    input::load_blocking,
    reconstruction::reconstruct_blocking,
    registration::register_blocking,
    slice::write_slice_blocking,
    territory::territory_blocking,
    tiles::write_tiles_blocking,
};
use tracing::info;

fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    dispatch(&args)
}

fn dispatch(args: &[String]) -> Result<(), Box<dyn Error>> {
    if boundary_command(args)? {
        return Ok(());
    }
    match args {
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
        [command, root, request, destination] if command == "territory" => {
            let files =
                territory_blocking(Path::new(root), Path::new(request), Path::new(destination))?;
            info!(files, destination, "Node territory compiled");
        }
        [command, root, ticks, selection, destination] if command == "slice" => {
            let files =
                write_slice_blocking(Path::new(root), ticks, selection, Path::new(destination))?;
            info!(files, ticks, selection, destination, "Time slice compiled");
        }
        [command, root, base_url, maxzoom, destination] if command == "tiles" => {
            let files = write_tiles_blocking(
                Path::new(root),
                base_url,
                maxzoom.parse()?,
                Path::new(destination),
            )?;
            info!(files, destination, "Vector tiles compiled");
        }
        _ => {
            return Err(
                "usage: atlas-compile verify <data> | build <data> <new-output-directory> | derive <data> <request.json> <new-output-directory> | register <data> <controls.json> <new-result.json> | slice <data> <tt-ms> <nominal|possible|certain> <new-output-directory> | reconstruct <data> <request.json> <new-output-directory> | territory <data> <request.json> <new-output-directory> | trace <data> <trace.json> <new-output-directory> | boundary <data> <trace.json> <controls.json> <new-output-directory> | tiles <data> <base-url> <max-zoom> <new-output-directory>".into(),
            );
        }
    }
    Ok(())
}

fn boundary_command(args: &[String]) -> Result<bool, Box<dyn Error>> {
    match args {
        [command, root, request, destination] if command == "derive" => {
            let files =
                derive_blocking(Path::new(root), Path::new(request), Path::new(destination))?;
            info!(files, destination, "Terrain boundaries reconstructed");
        }
        [command, root, trace, destination] if command == "trace" => {
            let files = boundary_blocking(
                Path::new(root),
                Path::new(trace),
                None,
                Path::new(destination),
            )?;
            info!(files, destination, "Map trace compiled");
        }
        [command, root, trace, controls, destination] if command == "boundary" => {
            let files = boundary_blocking(
                Path::new(root),
                Path::new(trace),
                Some(Path::new(controls)),
                Path::new(destination),
            )?;
            info!(files, destination, "Map boundary registered");
        }
        _ => return Ok(false),
    }
    Ok(true)
}
