//! Syntax-aware repository structure checks.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use syn::visit::Visit;
use tracing::{error, info};

mod syntax;

fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .init();
    let mut paths = Vec::new();
    collect_blocking(Path::new("."), &mut paths)?;
    let mut failures = Vec::new();
    for path in &paths {
        let content = fs::read_to_string(path)?;
        let syntax = syn::parse_file(&content)?;
        let mut checker = syntax::Checker::default();
        checker.visit_file(&syntax);
        check_file(path, &content, &syntax, &mut checker.problems);
        for problem in checker.problems {
            failures.push(format!("{}: {problem}", path.display()));
        }
    }
    for failure in &failures {
        error!("{failure}");
    }
    if !failures.is_empty() {
        return Err(format!("{} structure violations", failures.len()).into());
    }
    info!(files = paths.len(), "Rust structure verified");
    Ok(())
}

fn check_file(path: &Path, content: &str, syntax: &syn::File, problems: &mut Vec<String>) {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let limit = if name == "lib.rs" { 100 } else { 500 };
    if content.lines().count() > limit {
        problems.push(format!("file exceeds {limit} lines"));
    }
    if ["mod.rs", "utils.rs", "helpers.rs", "common.rs"].contains(&name) {
        problems.push(format!("prohibited module name {name}"));
    }
    if name == "lib.rs" {
        if !content.starts_with("//!") {
            problems.push("lib.rs must begin with crate documentation".into());
        }
        if syntax
            .items
            .iter()
            .any(|item| !matches!(item, syn::Item::Mod(_) | syn::Item::Use(_)))
        {
            problems.push("lib.rs may contain only module declarations and re-exports".into());
        }
    }
}

fn collect_blocking(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if ["target", "dist", ".git", "scratch", "tmp", "node_modules"]
            .iter()
            .any(|skip| name == *skip)
        {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_blocking(&path, paths)?;
        }
        if kind.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            paths.push(path);
        }
    }
    Ok(())
}
