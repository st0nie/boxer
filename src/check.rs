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

fn is_patch_file_name(name: &str) -> bool {
    !name.starts_with('.') && !name.ends_with('~') && name != "series"
}

fn needs_quilt_upgrade(text: &str) -> bool {
    text.contains("quilt upgrade") || text.contains("older version of quilt")
}

pub fn run_check(c: &Colors, args: &Args) -> Result<()> {
    crate::tools::ensure_installed(c, args, &["quilt"])?;
    check_patches(c)
}

pub(crate) fn check_patches(c: &Colors) -> Result<()> {
    info(c, "checking quilt patches");

    let patches_dir = Path::new("debian/patches");
    if !patches_dir.exists() {
        success(c, "no debian/patches directory, skipping");
        return Ok(());
    }

    let has_patches = fs::read_dir(patches_dir)?.filter_map(|e| e.ok()).any(|e| {
        let name = e.file_name();
        let name = name.to_string_lossy();
        e.file_type().ok().is_some_and(|t| t.is_file()) && is_patch_file_name(&name)
    });

    if !has_patches {
        success(c, "no patches found, skipping");
        return Ok(());
    }

    // If patches are already applied in the working tree, pop them first so
    // `quilt push -a` tests the full series from a clean unapplied state.
    let applied = output(&mut quilt(&["applied"]))?;
    if applied.status.success() {
        warning(c, "patches already applied, popping before check");
        run(&mut quilt(&["pop", "-a"]))?;
    }

    let out = output(&mut quilt(&["push", "-a"]))?;

    if !out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let combined = format!("{}{}", stdout, stderr);

        if needs_quilt_upgrade(&combined) {
            warning(c, "old quilt format detected, running quilt upgrade");
            run(&mut quilt(&["upgrade"]))?;
            success(c, "quilt upgrade completed");

            if let Err(e) = run(&mut quilt(&["push", "-a"])) {
                let _ = output(&mut quilt(&["pop", "-a"]));
                return Err(e);
            }
        } else {
            // Roll back any patches applied before the failure so the working
            // tree is not left in a partially patched state.
            let _ = output(&mut quilt(&["pop", "-a"]));
            bail!("quilt push -a failed: {}", combined.trim());
        }
    }

    success(c, "quilt push -a succeeded");

    run(&mut quilt(&["pop", "-a"]))?;
    success(c, "quilt pop -a succeeded");

    success(c, "patch check passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_patch_file_names() {
        assert!(is_patch_file_name("0001-fix.patch"));
        assert!(!is_patch_file_name("series"));
        assert!(!is_patch_file_name(".pc"));
        assert!(!is_patch_file_name("0001-fix.patch~"));
    }

    #[test]
    fn detects_quilt_upgrade_diagnostics() {
        assert!(needs_quilt_upgrade(
            "The working tree was created by an older version of quilt. Please run 'quilt upgrade'."
        ));
        assert!(!needs_quilt_upgrade("Patch 0001.patch does not apply"));
    }
}
