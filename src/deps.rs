use crate::args::Args;
use crate::debian::{
    clean_deps, get_build_depends_from_apt, get_source_info, parse_build_depends,
};
use crate::exec::{null_stdout, run, SUDO_BIN};
use crate::fmt::{detail, info, success};
use crate::Colors;

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run_deps(c: &Colors, args: &Args, package: Option<&str>) -> Result<()> {
    let verbose = args.verbose >= 1;

    info(c, "resolving build dependencies");
    let (source, _) = get_source_info()?;
    let target = package.unwrap_or(&source);
    let meta_pkg = format!("{}-build-deps", target);
    success(c, &format!("target package: {}", target));

    let raw_deps = if Path::new("debian/control").exists() {
        detail(c, "reading build dependencies from debian/control");
        parse_build_depends(Path::new("debian/control"))?
    } else {
        detail(c, "falling back to apt-cache showsrc");
        get_build_depends_from_apt(target)?
    };

    if raw_deps.trim().is_empty() {
        bail!("no build dependencies found");
    }

    detail(c, "cleaning dependency syntax");
    let cleaned = clean_deps(&raw_deps);
    if cleaned.is_empty() {
        bail!("no build dependencies left after cleaning");
    }
    success(c, &format!("{} build dependencies", cleaned.split(", ").count()));
    if verbose {
        detail(c, &cleaned);
    }

    let deb_file = build_equivs_package(c, &meta_pkg, target, &cleaned)?;
    success(c, &format!("generated {}", deb_file.display()));

    info(c, "installing build-dep meta package");
    run(
        "apt",
        &["install", &format!("./{}", deb_file.display()), "-y"],
        true,
        SUDO_BIN,
    )?;
    success(c, "build-dep meta package installed");

    run("apt-mark", &["auto", &meta_pkg], true, SUDO_BIN)?;
    success(c, "meta package marked as auto");

    info(c, "cleaning local build-dep artifacts");
    fs::remove_file(&deb_file).ok();
    fs::remove_file(format!("{}-equivs", meta_pkg)).ok();

    success(c, "build dependencies installed and marked as auto");
    Ok(())
}

fn build_equivs_package(c: &Colors, meta_pkg: &str, target_pkg: &str, deps: &str) -> Result<PathBuf> {
    let equivs_cfg = format!("{}-equivs", meta_pkg);

    info(c, "generating build-dep meta package");
    let content = format!(
        "Section: misc\n\
         Priority: optional\n\
         Package: {}\n\
         Version: 1.0\n\
         Architecture: all\n\
         Depends: {}\n\
         Description: Automatically generated build dependencies for {}\n \
         This virtual/meta package depends on all packages required to build {}.\n \
         Uninstalling this package and running 'apt autoremove' will cleanly restore your environment.\n",
        meta_pkg, deps, target_pkg, target_pkg
    );
    fs::write(&equivs_cfg, content)
        .context("failed to write equivs config")?;

    let tmp = tempfile::tempdir().context("failed to create temp dir")?;
    fs::copy(&equivs_cfg, tmp.path().join(&equivs_cfg))
        .context("failed to copy equivs config to temp dir")?;

    detail(c, "running equivs-build");
    let mut cmd = Command::new("equivs-build");
    cmd.arg(&equivs_cfg).current_dir(tmp.path());
    null_stdout(&mut cmd);

    let status = cmd.status().context("failed to run equivs-build")?;
    if !status.success() {
        bail!("equivs-build failed");
    }

    let deb_name = format!("{}_1.0_all.deb", meta_pkg);
    let generated = tmp.path().join(&deb_name);
    if !generated.exists() {
        bail!("expected deb file not found: {}", deb_name);
    }

    let local_deb = PathBuf::from(&deb_name);
    fs::copy(&generated, &local_deb).context("failed to copy generated deb to current dir")?;

    Ok(local_deb)
}
