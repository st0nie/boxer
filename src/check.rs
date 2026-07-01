use crate::args::Args;
use crate::exec::{run_env, run_env_capture};
use crate::fmt::{info, success, warning};
use crate::Colors;

use anyhow::{bail, Context, Result};
use std::path::Path;

pub fn run_check(c: &Colors, _args: &Args) -> Result<()> {
    info(c, "checking quilt patches");

    let patches_dir = Path::new("debian/patches");
    if !patches_dir.exists() {
        success(c, "no debian/patches directory, skipping");
        return Ok(());
    }

    let has_patches = fs_err::read_dir(patches_dir)?
        .filter_map(|e| e.ok())
        .any(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            e.file_type().map(|t| t.is_file()).unwrap_or(false)
                && !name.starts_with('.')
                && !name.ends_with("~")
                && name != "series"
        });

    if !has_patches {
        success(c, "no patches found, skipping");
        return Ok(());
    }

    let output = run_env_capture("quilt", &["push", "-a"], "QUILT_PATCHES", "debian/patches")
        .context("quilt push -a failed")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}{}", stdout, stderr);

        if combined.contains("quilt upgrade") || combined.contains("older version of quilt") {
            warning(c, "old quilt format detected, running quilt upgrade");
            run_env("quilt", &["upgrade"], "QUILT_PATCHES", "debian/patches")?;
            success(c, "quilt upgrade completed");

            run_env("quilt", &["push", "-a"], "QUILT_PATCHES", "debian/patches")?;
        } else {
            bail!("quilt push -a failed: {}", combined.trim());
        }
    }

    success(c, "quilt push -a succeeded");

    run_env("quilt", &["pop", "-a"], "QUILT_PATCHES", "debian/patches")?;
    success(c, "quilt pop -a succeeded");

    success(c, "patch check passed");
    Ok(())
}
