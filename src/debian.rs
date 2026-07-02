use crate::exec::command_output;

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn get_source_info() -> Result<(String, String)> {
    let output = command_output(Command::new("dpkg-parsechangelog").arg("-ldebian/changelog"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut source = None;
    let mut version = None;

    for line in stdout.lines() {
        if let Some(v) = line.strip_prefix("Source: ") {
            source = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Version: ") {
            version = Some(v.trim().to_string());
        }
    }

    let source = source.context("could not determine source package name from debian/changelog")?;
    let version = version.context("could not determine version from debian/changelog")?;
    Ok((source, version))
}

pub fn collect_files_by_ext(dir: &Path, exts: &[&str]) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read dir: {:?}", dir))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if exts.iter().any(|ext| name.ends_with(ext)) {
            files.push(entry.path());
        }
    }
    Ok(files)
}
