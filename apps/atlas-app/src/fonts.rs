//! Publication font inventory shared by native and browser hosts.

use std::path::{Path, PathBuf};

/// A font inventory or its asset cannot be loaded.
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    /// Inventory JSON is invalid.
    #[error("font inventory: {0}")]
    Json(#[from] serde_json::Error),
    /// Font name is not a local asset filename.
    #[error("invalid font asset name {0:?}")]
    Name(String),
    /// Native font asset read failed.
    #[error("font asset {}: {source}", path.display())]
    Io {
        /// Requested asset path.
        path: PathBuf,
        /// Filesystem failure.
        #[source]
        source: std::io::Error,
    },
}

/// Parse an ordered list of font files without restricting writing systems.
pub fn names(bytes: &[u8]) -> Result<Vec<String>, FontError> {
    let names: Vec<String> = serde_json::from_slice(bytes)?;
    for name in &names {
        if name.is_empty()
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
            || name.contains("..")
            || !(name.ends_with(".otf") || name.ends_with(".ttf"))
        {
            return Err(FontError::Name(name.clone()));
        }
    }
    Ok(names)
}

/// Install publication fonts as fallbacks to egui's Latin and symbol coverage.
pub fn install(context: &egui::Context, files: Vec<(String, Vec<u8>)>) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, data) in files {
        fonts
            .font_data
            .insert(name.clone(), egui::FontData::from_owned(data).into());
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
    }
    context.set_fonts(fonts);
}

/// Load a native publication's fonts from its asset directory.
pub fn configure_blocking(context: &egui::Context, directory: &Path) -> Result<(), FontError> {
    let read =
        |path: PathBuf| std::fs::read(&path).map_err(|source| FontError::Io { path, source });
    let mut files = Vec::new();
    for name in names(&read(directory.join("fonts.json"))?)? {
        let bytes = read(directory.join(&name))?;
        files.push((name, bytes));
    }
    install(context, files);
    Ok(())
}

#[cfg(test)]
mod tests;
