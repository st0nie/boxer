use crate::Colors;
use crate::args::Args;
use crate::debian::{collect_files_by_ext, get_source_info};
use crate::deps::run_deps;
use crate::exec::run;
use crate::fmt::{detail, info, success};

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run_build(c: &Colors, args: &Args) -> Result<()> {
    crate::tools::ensure_installed(
        c,
        args,
        &[
            "quilt",
            "dpkg-parsechangelog",
            "mk-build-deps",
            "dpkg-buildpackage",
        ],
    )?;
    info(c, "starting build pipeline");

    crate::check::run_check(c, args)?;
    run_deps(c, args, None)?;

    let (source, version) = get_source_info()?;
    success(c, &format!("building {} {}", source, version));

    info(c, "running dpkg-buildpackage");
    let mut cmd = Command::new("dpkg-buildpackage");
    cmd.args(["-b", "-uc", "-us"])
        .env("DEB_BUILD_OPTIONS", "nocheck");
    run(&mut cmd)?;
    success(c, "dpkg-buildpackage finished");

    let current = std::env::current_dir()?;
    let parent = Path::new("..").canonicalize()?;

    info(c, "collecting generated debs");
    let debs = collect_files_by_ext(&parent, &[".deb", ".udeb"])?;
    if debs.is_empty() {
        bail!("no .deb files were generated");
    }
    for src in &debs {
        let dst = current.join(src.file_name().unwrap());
        move_file(src, &dst).with_context(|| format!("failed to move {:?} to {:?}", src, dst))?;
        success(c, &format!("moved {}", dst.display()));
    }

    let local_debs = collect_files_by_ext(&current, &[".deb", ".udeb"])?;
    let local_names: Vec<String> = local_debs
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    let tarball = format!("{}-{}.tar.gz", source, version);
    info(c, &format!("creating tarball: {}", tarball));

    let mut tar_args = vec![
        "--force-local".to_string(),
        "-czf".to_string(),
        tarball.clone(),
    ];
    tar_args.extend(local_names.iter().cloned());
    let mut cmd = Command::new("tar");
    cmd.args(&tar_args);
    run(&mut cmd)?;
    success(c, &format!("created {}", tarball));

    info(c, "cleaning generated files");
    for name in &local_names {
        fs::remove_file(name).context("failed to remove local deb")?;
    }
    clean_parent_build_artifacts(c)?;

    success(c, &format!("build complete: {}", tarball));
    Ok(())
}

/// Move a file, falling back to copy+remove when source and destination live
/// on different filesystems (`rename` fails with `EXDEV` in that case).
fn move_file(src: &Path, dst: &Path) -> Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            fs::copy(src, dst).with_context(|| format!("failed to copy {:?} to {:?}", src, dst))?;
            fs::remove_file(src).with_context(|| format!("failed to remove {:?}", src))?;
            Ok(())
        }
        Err(e) => Err(anyhow::Error::new(e)),
    }
}

fn clean_parent_build_artifacts(c: &Colors) -> Result<()> {
    let parent = Path::new("..").canonicalize()?;
    let exts = [".deb", ".udeb", ".buildinfo", ".changes"];
    for path in collect_files_by_ext(&parent, &exts)? {
        detail(c, &format!("removing {}", path.display()));
        fs::remove_file(&path).with_context(|| format!("failed to remove {:?}", path))?;
    }
    Ok(())
}
