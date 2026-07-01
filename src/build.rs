use crate::args::Args;
use crate::debian::{collect_files_by_ext, get_source_info};
use crate::deps::run_deps;
use crate::exec::{run, run_env, SUDO_BIN};
use crate::fmt::{detail, info, success};
use crate::Colors;

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

pub fn run_build(c: &Colors, args: &Args) -> Result<()> {
    let _verbose = args.verbose >= 1;

    info(c, "starting build pipeline");

    crate::check::run_check(c, args)?;
    run_deps(c, args, None)?;

    let (source, version) = get_source_info()?;
    success(c, &format!("building {} {}", source, version));

    info(c, "running dpkg-buildpackage");
    run_env(
        "dpkg-buildpackage",
        &["-b", "-uc", "-us"],
        "DEB_BUILD_OPTIONS",
        "nocheck",
    )?;
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
        fs::rename(src, &dst).with_context(|| format!("failed to move {:?} to {:?}", src, dst))?;
        success(c, &format!("moved {}", dst.display()));
    }

    let local_debs = collect_files_by_ext(&current, &[".deb", ".udeb"])?;
    let local_names: Vec<String> = local_debs
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    let tarball = format!("{}-{}.tar.gz", source, version);
    info(c, &format!("creating tarball: {}", tarball));

    let mut args = vec!["czf", &tarball];
    args.extend(local_names.iter().map(|s| s.as_str()));
    run("tar", &args, false, SUDO_BIN)?;
    success(c, &format!("created {}", tarball));

    info(c, "cleaning generated files");
    for name in &local_names {
        fs::remove_file(name).context("failed to remove local deb")?;
    }
    clean_parent_build_artifacts(c)?;

    success(c, &format!("build complete: {}", tarball));
    Ok(())
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
