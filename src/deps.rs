use crate::args::Args;
use crate::debian::{clean_deps, get_source_info, parse_build_depends};
use crate::exec::{run, run_env, SUDO_BIN};
use crate::fmt::{detail, info, success};
use crate::Colors;

use anyhow::{bail, Result};
use std::path::Path;

pub fn run_deps(c: &Colors, args: &Args, package: Option<&str>) -> Result<()> {
    let verbose = args.verbose >= 1;

    info(c, "resolving build dependencies");
    let (source, _) = get_source_info()?;
    let target = package.unwrap_or(&source);
    success(c, &format!("target package: {}", target));

    if !Path::new("debian/control").exists() {
        bail!("debian/control not found; boxer deps only works in a Debian source tree");
    }

    detail(c, "reading build dependencies from debian/control");
    let raw_deps = parse_build_depends(Path::new("debian/control"))?;

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

    info(c, "installing build dependencies via mk-build-deps");
    run_env(
        "mk-build-deps",
        &[
            "--install",
            "--root-cmd",
            SUDO_BIN,
            "--remove",
            "--tool",
            "apt-get -o Debug::pkgProblemResolver=yes --no-install-recommends -y",
            "debian/control",
        ],
        "DEBIAN_FRONTEND",
        "noninteractive",
    )?;

    let meta_pkg = format!("{}-build-deps", target);
    run("apt-mark", &["auto", &meta_pkg], true, SUDO_BIN)?;

    success(c, "build dependencies installed and marked as auto");
    Ok(())
}
