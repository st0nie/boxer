use crate::Colors;
use crate::args::Args;
use crate::debian::{
    BuildArtifacts, collect_files, ensure_source_tree, get_binary_packages, get_changes_binaries,
    get_source_info,
};
use crate::exec::run;
use crate::fmt::{detail, info, success};

use anyhow::{Context, Result, bail};
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run_build(c: &Colors, args: &Args) -> Result<()> {
    ensure_source_tree()?;

    let _root = crate::exec::acquire_root()?;
    crate::tools::ensure_installed(
        c,
        args,
        &[
            "quilt",
            "dpkg-parsechangelog",
            "mk-build-deps",
            "equivs-build",
            "dpkg-buildpackage",
        ],
    )?;
    info(c, "starting build pipeline");

    crate::check::check_patches(c)?;

    let (source, version) = get_source_info()?;
    crate::deps::install_source_deps(c, &source)?;

    let parent = Path::new("..").canonicalize()?;
    let pre_artifacts = BuildArtifacts::new(&source, &version, &get_binary_packages()?);
    clean_parent_build_artifacts(c, &parent, &pre_artifacts)?;

    success(c, &format!("building {} {}", source, version));

    info(c, "running dpkg-buildpackage");
    let deb_build_options =
        merge_deb_build_options(std::env::var("DEB_BUILD_OPTIONS").ok().as_deref());
    let mut cmd = Command::new("dpkg-buildpackage");
    cmd.args(["-b", "-uc", "-us"])
        .env("DEB_BUILD_OPTIONS", deb_build_options);
    run(&mut cmd)?;
    success(c, "dpkg-buildpackage finished");

    // Read debian/control and any generated .changes file after the build so
    // dynamically generated or substvar-expanded binary packages are included.
    let mut artifacts = BuildArtifacts::new(&source, &version, &get_binary_packages()?);
    artifacts.add_binaries(&get_changes_binaries(&parent, &source, &version)?);

    info(c, "collecting generated debs");
    let mut deb_names: Vec<OsString> =
        collect_files(&parent, |name| artifacts.is_current_binary(name))?
            .into_iter()
            .filter_map(|p| p.file_name().map(|n| n.to_os_string()))
            .collect();
    deb_names.sort();
    if deb_names.is_empty() {
        bail!("no .deb files were generated for {} {}", source, version);
    }
    for name in &deb_names {
        detail(c, &format!("found {}", name.to_string_lossy()));
    }

    let tarball = format!("{}-{}.tar.gz", source, version);
    info(c, &format!("creating tarball: {}", tarball));

    let mut cmd = Command::new("tar");
    cmd.arg("--force-local")
        .arg("-C")
        .arg(&parent)
        .arg("-czf")
        .arg(&tarball)
        .args(&deb_names);
    run(&mut cmd)?;
    success(c, &format!("created {}", tarball));

    info(c, "cleaning generated files");
    clean_parent_build_artifacts(c, &parent, &artifacts)?;

    success(c, &format!("build complete: {}", tarball));
    Ok(())
}

/// Preserve existing `DEB_BUILD_OPTIONS` tokens while ensuring `nocheck` is set.
fn merge_deb_build_options(existing: Option<&str>) -> String {
    let Some(raw) = existing.map(str::trim).filter(|s| !s.is_empty()) else {
        return "nocheck".to_string();
    };
    if raw.split_whitespace().any(|token| token == "nocheck") {
        raw.to_string()
    } else {
        format!("{raw} nocheck")
    }
}

/// Remove this source package's generated artifacts (any version) from the
/// parent directory. Files belonging to other packages are left alone, which
/// matters when the parent directory is a shared workspace.
fn clean_parent_build_artifacts(
    c: &Colors,
    parent: &Path,
    artifacts: &BuildArtifacts,
) -> Result<()> {
    for path in collect_files(parent, |name| artifacts.is_source_artifact(name))? {
        detail(c, &format!("removing {}", path.display()));
        fs::remove_file(&path).with_context(|| format!("failed to remove {:?}", path))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_nocheck_into_deb_build_options() {
        assert_eq!(merge_deb_build_options(None), "nocheck");
        assert_eq!(merge_deb_build_options(Some("")), "nocheck");
        assert_eq!(merge_deb_build_options(Some("   ")), "nocheck");
        assert_eq!(merge_deb_build_options(Some("nocheck")), "nocheck");
        assert_eq!(
            merge_deb_build_options(Some("parallel=8 nostrip")),
            "parallel=8 nostrip nocheck"
        );
        assert_eq!(
            merge_deb_build_options(Some("parallel=8 nocheck nostrip")),
            "parallel=8 nocheck nostrip"
        );
    }
}
