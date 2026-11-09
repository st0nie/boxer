use crate::Colors;
use crate::args::Args;
use crate::debian::{ensure_source_tree, get_source_info, parse_source_package};
use crate::exec::{is_root, root_cmd, run, run_sudo};
use crate::fmt::{info, success};

use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run_deps(c: &Colors, args: &Args, package: Option<&str>) -> Result<()> {
    if package.is_none() {
        ensure_source_tree()?;
    }

    let _root = crate::exec::acquire_root()?;
    let required_tools: &[&str] = if package.is_none() {
        &["dpkg-parsechangelog", "mk-build-deps", "equivs-build"]
    } else {
        &["mk-build-deps", "equivs-build"]
    };
    crate::tools::ensure_installed(c, args, required_tools)?;

    info(c, "resolving build dependencies");
    let source = if Path::new("debian/changelog").is_file() {
        get_source_info().ok().map(|(s, _)| s)
    } else {
        None
    };
    let control_source = fs::read_to_string("debian/control")
        .ok()
        .and_then(|text| parse_source_package(&text));

    let (target, spec) = resolve_deps_target(
        package,
        source.as_deref().or(control_source.as_deref()),
        Path::new("debian/control").is_file(),
    );

    install_deps(c, &target, &spec)
}

pub(crate) fn install_source_deps(c: &Colors, source: &str) -> Result<()> {
    info(c, "resolving build dependencies");
    install_deps(c, source, "debian/control")
}

/// Determine `(target_package_name, mk_build_deps_argument)`.
fn resolve_deps_target(
    package: Option<&str>,
    local_source: Option<&str>,
    has_debian_control: bool,
) -> (String, String) {
    match package {
        None => {
            let source = local_source.unwrap_or("unknown").to_string();
            (source, "debian/control".to_string())
        }
        Some(pkg) if has_debian_control && local_source == Some(pkg) => {
            (pkg.to_string(), "debian/control".to_string())
        }
        Some(pkg) if Path::new(pkg).is_file() => {
            let target = fs::read_to_string(pkg)
                .ok()
                .and_then(|text| parse_source_package(&text))
                .unwrap_or_else(|| pkg.to_string());
            (target, pkg.to_string())
        }
        Some(pkg) => (pkg.to_string(), pkg.to_string()),
    }
}

fn install_deps(c: &Colors, target: &str, spec: &str) -> Result<()> {
    success(c, &format!("target package: {}", target));

    info(c, "installing build dependencies via mk-build-deps");
    let mut cmd = Command::new("mk-build-deps");
    cmd.arg("--install");
    if !is_root() {
        cmd.args(["--root-cmd", root_cmd()]);
    }
    cmd.args([
        "--remove",
        "--tool",
        "apt-get -o Debug::pkgProblemResolver=yes --no-install-recommends -y",
        spec,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_default_source_tree_target() {
        assert_eq!(
            resolve_deps_target(None, Some("boxer"), true),
            ("boxer".to_string(), "debian/control".to_string())
        );
    }

    #[test]
    fn resolves_explicit_package_matching_local_source() {
        assert_eq!(
            resolve_deps_target(Some("boxer"), Some("boxer"), true),
            ("boxer".to_string(), "debian/control".to_string())
        );
    }

    #[test]
    fn resolves_external_package_name() {
        assert_eq!(
            resolve_deps_target(Some("other-pkg"), Some("boxer"), true),
            ("other-pkg".to_string(), "other-pkg".to_string())
        );
    }
}
