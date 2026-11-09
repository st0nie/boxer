use crate::exec::command_output;

use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Verify that the current directory is the root of a Debian source tree
/// (`debian/control` and `debian/changelog` are present).
pub fn ensure_source_tree() -> Result<()> {
    ensure_source_tree_in(Path::new("."))
}

fn ensure_source_tree_in(root: &Path) -> Result<()> {
    if !root.join("debian/control").is_file() {
        bail!("debian/control not found; run boxer from the root of a Debian source tree");
    }
    if !root.join("debian/changelog").is_file() {
        bail!("debian/changelog not found; run boxer from the root of a Debian source tree");
    }
    Ok(())
}

pub fn get_source_info() -> Result<(String, String)> {
    let output = command_output(Command::new("dpkg-parsechangelog").arg("-ldebian/changelog"))?;
    parse_changelog_output(&String::from_utf8_lossy(&output.stdout))
}

fn parse_changelog_output(stdout: &str) -> Result<(String, String)> {
    let mut source = None;
    let mut version = None;

    for line in stdout.lines() {
        if let Some(v) = line.strip_prefix("Source: ") {
            source = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Version: ") {
            version = Some(v.trim().to_string());
        }
    }

    let source = source.context("could not determine source package name from debian/changelog")?;
    let version = version.context("could not determine version from debian/changelog")?;
    Ok((source, version))
}

/// Extract the `Source:` package name from a `debian/control` file.
pub fn parse_source_package(control: &str) -> Option<String> {
    control
        .lines()
        .filter(|line| !line.starts_with([' ', '\t', '#']))
        .filter_map(|line| line.split_once(':'))
        .find(|(field, _)| field.trim().eq_ignore_ascii_case("source"))
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Binary package names declared in `debian/control`.
pub fn get_binary_packages() -> Result<Vec<String>> {
    let control = fs::read_to_string("debian/control").context("could not read debian/control")?;
    Ok(parse_binary_packages(&control))
}

/// Extract the `Package:` fields from a `debian/control` file.
///
/// Field names are case-insensitive and continuation lines start with
/// whitespace, so only the first line of each field is considered. The source
/// stanza uses `Source:` instead, hence every `Package:` field names a binary
/// package.
fn parse_binary_packages(control: &str) -> Vec<String> {
    control
        .lines()
        .filter(|line| !line.starts_with([' ', '\t', '#']))
        .filter_map(|line| line.split_once(':'))
        .filter(|(field, _)| field.trim().eq_ignore_ascii_case("package"))
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

/// Binary package names recorded in `<source>_<version>_*.changes` files in `dir`.
///
/// Reading the generated `.changes` file complements `debian/control` when binary
/// package names use substitution variables or are generated dynamically during
/// the build.
pub fn get_changes_binaries(dir: &Path, source: &str, version: &str) -> Result<Vec<String>> {
    let build_version = strip_epoch(version);
    let changes_files = collect_files(dir, |name| {
        name.ends_with(".changes")
            && split_artifact_name(name)
                .is_some_and(|(pkg, ver)| pkg == source && ver == build_version)
    })?;

    let mut binaries = Vec::new();
    for path in changes_files {
        if let Ok(text) = fs::read_to_string(&path) {
            binaries.extend(parse_changes_binaries(&text));
        }
    }
    Ok(binaries)
}

/// Extract binary package names from a Debian `.changes` file (`Binary:` header
/// and `Files:` / `Checksums-*:` file entries).
fn parse_changes_binaries(changes: &str) -> Vec<String> {
    let mut packages = Vec::new();
    for line in changes.lines() {
        if let Some((field, value)) = line.split_once(':')
            && !line.starts_with([' ', '\t'])
            && field.trim().eq_ignore_ascii_case("binary")
        {
            for pkg in value.split_whitespace() {
                if !pkg.is_empty() && !packages.iter().any(|p| p == pkg) {
                    packages.push(pkg.to_string());
                }
            }
        } else if line.starts_with([' ', '\t'])
            && let Some(file_name) = line.split_whitespace().last()
            && is_binary_package(file_name)
            && let Some((pkg, _)) = split_artifact_name(file_name)
            && !packages.iter().any(|p| p == pkg)
        {
            packages.push(pkg.to_string());
        }
    }
    packages
}

/// Collect files in `dir` whose name satisfies `predicate`.
pub fn collect_files<F>(dir: &Path, predicate: F) -> Result<Vec<PathBuf>>
where
    F: Fn(&str) -> bool,
{
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read dir: {:?}", dir))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        if predicate(&entry.file_name().to_string_lossy()) {
            files.push(entry.path());
        }
    }
    Ok(files)
}

/// Selection of generated Debian artifacts that belong to a source package.
///
/// The binary package names come from `debian/control` (and `.changes` when
/// available) and may differ from the source name (`systemd` also builds
/// `libpam-systemd`), so they cannot be derived from the source name alone.
/// Debug symbol packages are generated by `dh_strip` as `<binary>-dbgsym` and
/// never appear in `debian/control`, so those companions are added explicitly.
pub struct BuildArtifacts {
    source: String,
    /// Version as written into file names (dpkg drops the epoch).
    build_version: String,
    packages: HashSet<String>,
}

impl BuildArtifacts {
    pub fn new(source: &str, version: &str, binary_packages: &[String]) -> Self {
        let mut packages: HashSet<String> = HashSet::new();
        packages.insert(source.to_string());
        packages.extend(binary_packages.iter().cloned());
        let dbgsym = packages
            .iter()
            .map(|package| format!("{package}-dbgsym"))
            .collect::<Vec<_>>();
        packages.extend(dbgsym);

        Self {
            source: source.to_string(),
            build_version: strip_epoch(version).to_string(),
            packages,
        }
    }

    /// Register additional binary package names (e.g. discovered from `.changes`)
    /// and their `-dbgsym` companions.
    pub fn add_binaries(&mut self, extra: &[String]) {
        for pkg in extra {
            self.packages.insert(pkg.clone());
            self.packages.insert(format!("{pkg}-dbgsym"));
        }
    }

    /// `true` when `file_name` is a `.deb`/`.udeb`/`.ddeb` produced by this build.
    pub fn is_current_binary(&self, file_name: &str) -> bool {
        is_binary_package(file_name)
            && split_artifact_name(file_name).is_some_and(|(package, version)| {
                version == self.build_version && self.packages.contains(package)
            })
    }

    /// `true` when `file_name` is a `.deb`, `.udeb`, `.ddeb`, `.buildinfo`, or
    /// `.changes` belonging to this source package, regardless of version.
    pub fn is_source_artifact(&self, file_name: &str) -> bool {
        let Some((package, _)) = split_artifact_name(file_name) else {
            return false;
        };
        if is_binary_package(file_name) {
            self.packages.contains(package)
        } else {
            package == self.source
                && (file_name.ends_with(".buildinfo") || file_name.ends_with(".changes"))
        }
    }
}

/// Version as it appears in generated file names: dpkg drops the epoch
/// (`1:2.0-1` is written as `2.0-1`).
fn strip_epoch(version: &str) -> &str {
    version.split_once(':').map_or(version, |(_, rest)| rest)
}

/// `true` when `file_name` is a binary package file (`.deb`, `.udeb`, or `.ddeb`).
fn is_binary_package(file_name: &str) -> bool {
    file_name.ends_with(".deb") || file_name.ends_with(".udeb") || file_name.ends_with(".ddeb")
}

/// Split a Debian artifact name (`package_version_arch.ext`) into its package
/// and version components. Package and version names cannot contain
/// underscores, so the fields are unambiguous.
fn split_artifact_name(file_name: &str) -> Option<(&str, &str)> {
    let mut fields = file_name.splitn(3, '_');
    let package = fields.next()?;
    let version = fields.next()?;
    let remainder = fields.next()?;
    if package.is_empty() || version.is_empty() || remainder.is_empty() {
        return None;
    }
    Some((package, version))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn parses_source_and_package_fields_from_control() {
        let control = "\
# Comment: not a field
Source: foo
Section: misc

Package: foo
Architecture: any
Description: foo
 .
 Continuation mentioning Package: not-a-field

Package: libfoo2
Architecture: any

package: foo-tools
";
        assert_eq!(parse_source_package(control), Some("foo".to_string()));
        let packages = parse_binary_packages(control);
        assert_eq!(packages, names(&["foo", "libfoo2", "foo-tools"]));
    }

    #[test]
    fn parses_changelog_output_fields() {
        let out = "Source: boxer\nVersion: 1:0.2.0-1\nDistribution: unstable\n";
        assert_eq!(
            parse_changelog_output(out).unwrap(),
            ("boxer".to_string(), "1:0.2.0-1".to_string())
        );
        assert!(parse_changelog_output("Source: boxer\n").is_err());
    }

    #[test]
    fn parses_binaries_from_changes_file() {
        let changes = "\
Format: 1.8
Source: foo
Binary: foo libfoo-abi1
Version: 1.0-1
Files:
 d41d8cd98f00b204e9800998ecf8427e 1234 misc optional foo_1.0-1_amd64.deb
 d41d8cd98f00b204e9800998ecf8427e 5678 debug optional foo-dbgsym_1.0-1_amd64.ddeb
 d41d8cd98f00b204e9800998ecf8427e 9012 libs optional libfoo-extra_1.0-1_amd64.deb
 d41d8cd98f00b204e9800998ecf8427e 345 misc optional foo_1.0-1_amd64.buildinfo
";
        let pkgs = parse_changes_binaries(changes);
        assert_eq!(
            pkgs,
            names(&["foo", "libfoo-abi1", "foo-dbgsym", "libfoo-extra"])
        );
    }

    #[test]
    fn current_build_matches_source_version() {
        let mut artifacts = BuildArtifacts::new("foo", "1.0-1", &names(&["foo"]));
        artifacts.add_binaries(&names(&["libfoo-dynamic"]));
        assert!(artifacts.is_current_binary("foo_1.0-1_amd64.deb"));
        assert!(artifacts.is_current_binary("foo_1.0-1_all.udeb"));
        assert!(artifacts.is_current_binary("foo-dbgsym_1.0-1_amd64.deb"));
        assert!(artifacts.is_current_binary("foo-dbgsym_1.0-1_amd64.ddeb"));
        assert!(artifacts.is_current_binary("libfoo-dynamic_1.0-1_amd64.deb"));
        assert!(artifacts.is_current_binary("libfoo-dynamic-dbgsym_1.0-1_amd64.ddeb"));
        assert!(!artifacts.is_current_binary("foo_1.0-2_amd64.deb"));
        assert!(!artifacts.is_current_binary("foo_0.9-1_amd64.deb"));
        assert!(!artifacts.is_current_binary("otherpkg_1.0-1_amd64.deb"));
        assert!(!artifacts.is_current_binary("foo_1.0-1_amd64.buildinfo"));
        assert!(!artifacts.is_current_binary("foo-1.0-1.tar.gz"));
    }

    #[test]
    fn current_build_matches_epoch_stripped_file_names() {
        let artifacts = BuildArtifacts::new("foo", "1:2.0-1", &names(&["foo"]));
        assert!(artifacts.is_current_binary("foo_2.0-1_amd64.deb"));
        assert!(!artifacts.is_current_binary("foo_1.0-1_amd64.deb"));
    }

    #[test]
    fn current_build_covers_control_binaries() {
        let artifacts =
            BuildArtifacts::new("systemd", "257-1", &names(&["systemd", "libpam-systemd"]));
        assert!(artifacts.is_current_binary("systemd_257-1_amd64.deb"));
        assert!(artifacts.is_current_binary("libpam-systemd_257-1_amd64.deb"));
        assert!(artifacts.is_current_binary("libpam-systemd-dbgsym_257-1_amd64.deb"));
        assert!(!artifacts.is_current_binary("libsystemd0_257-1_amd64.deb"));
    }

    #[test]
    fn source_artifacts_ignore_version() {
        let artifacts = BuildArtifacts::new("foo", "1.0-1", &names(&["foo", "libfoo2"]));
        assert!(artifacts.is_source_artifact("foo_1.0-1_amd64.deb"));
        assert!(artifacts.is_source_artifact("foo_0.9-1_amd64.deb"));
        assert!(artifacts.is_source_artifact("libfoo2_0.9-1_amd64.deb"));
        assert!(artifacts.is_source_artifact("libfoo2-dbgsym_0.9-1_amd64.deb"));
        assert!(artifacts.is_source_artifact("libfoo2-dbgsym_0.9-1_amd64.ddeb"));
        assert!(artifacts.is_source_artifact("foo_1.0-1_amd64.buildinfo"));
        assert!(artifacts.is_source_artifact("foo_1.0-1_amd64.changes"));
        assert!(!artifacts.is_source_artifact("otherpkg_1.0-1_amd64.deb"));
        assert!(!artifacts.is_source_artifact("libbar_1.0-1_amd64.changes"));
        assert!(!artifacts.is_source_artifact("foo-1.0-1.tar.gz"));
        assert!(!artifacts.is_source_artifact("foo_1.0-1_amd64.dsc"));
    }

    #[test]
    fn ensure_source_tree_validates_control_and_changelog() {
        let dir = std::env::temp_dir().join(format!("boxer-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("debian")).unwrap();

        assert!(ensure_source_tree_in(&dir).is_err());
        fs::write(dir.join("debian/control"), "Source: foo\n").unwrap();
        assert!(ensure_source_tree_in(&dir).is_err());
        fs::write(
            dir.join("debian/changelog"),
            "foo (1.0-1) unstable; urgency=medium\n",
        )
        .unwrap();
        assert!(ensure_source_tree_in(&dir).is_ok());

        let _ = fs::remove_dir_all(&dir);
    }
}
