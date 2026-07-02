use crate::Colors;
use crate::args::Args;
use crate::debian::get_source_info;
use crate::exec::{run, run_sudo};
use crate::fmt::{info, success};

use anyhow::{Result, bail};
use std::path::Path;
use std::process::Command;

pub fn run_deps(c: &Colors, args: &Args, package: Option<&str>) -> Result<()> {
    crate::tools::ensure_installed(c, args, &["dpkg-parsechangelog", "mk-build-deps"])?;
    info(c, "resolving build dependencies");
    let (source, _) = get_source_info()?;
    let target = package.unwrap_or(&source);
    success(c, &format!("target package: {}", target));

    if !Path::new("debian/control").exists() {
        bail!("debian/control not found; boxer deps only works in a Debian source tree");
    }

    info(c, "installing build dependencies via mk-build-deps");
    let mut cmd = Command::new("mk-build-deps");
    cmd.args([
        "--install",
        "--root-cmd",
        "sudo",
        "--remove",
        "--tool",
        "apt-get -o Debug::pkgProblemResolver=yes --no-install-recommends -y",
        "debian/control",
    ])
    .env("DEBIAN_FRONTEND", "noninteractive");
    run(&mut cmd)?;

    let meta_pkg = format!("{}-build-deps", target);
    let mut cmd = Command::new("apt-mark");
    cmd.args(["auto", &meta_pkg]);
    run_sudo(&mut cmd)?;

    success(c, "build dependencies installed and marked as auto");
    Ok(())
}
