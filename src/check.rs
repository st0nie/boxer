use crate::Colors;
use crate::args::Args;
use crate::exec::{output, run};
use crate::fmt::{info, success, warning};

use anyhow::{Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;

fn quilt(args: &[&str]) -> Command {
    let mut cmd = Command::new("quilt");
    cmd.args(args).env("QUILT_PATCHES", "debian/patches");
    cmd
}

pub fn run_check(c: &Colors, _args: &Args) -> Result<()> {
    info(c, "checking quilt patches");

    let patches_dir = Path::new("debian/patches");
    if !patches_dir.exists() {
        success(c, "no debian/patches directory, skipping");
        return Ok(());
    }

    let has_patches = fs::read_dir(patches_dir)?.filter_map(|e| e.ok()).any(|e| {
        let name = e.file_name().to_string_lossy().to_string();
        e.file_type().map(|t| t.is_file()).unwrap_or(false)
            && !name.starts_with('.')
            && !name.ends_with('~')
            && name != "series"
    });

    if !has_patches {
        success(c, "no patches found, skipping");
        return Ok(());
    }

    let out = output(&mut quilt(&["push", "-a"]))?;

    if !out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let combined = format!("{}{}", stdout, stderr);

        if combined.contains("quilt upgrade") || combined.contains("older version of quilt") {
            warning(c, "old quilt format detected, running quilt upgrade");
            run(&mut quilt(&["upgrade"]))?;
            success(c, "quilt upgrade completed");

            run(&mut quilt(&["push", "-a"]))?;
        } else {
            bail!("quilt push -a failed: {}", combined.trim());
        }
    }

    success(c, "quilt push -a succeeded");

    run(&mut quilt(&["pop", "-a"]))?;
    success(c, "quilt pop -a succeeded");

    success(c, "patch check passed");
    Ok(())
}
