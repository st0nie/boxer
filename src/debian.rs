use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn get_source_info() -> Result<(String, String)> {
    let output = Command::new("dpkg-parsechangelog")
        .arg("-ldebian/changelog")
        .output()
        .context("failed to run dpkg-parsechangelog")?;

    if !output.status.success() {
        bail!(
            "dpkg-parsechangelog failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
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

/// Extract a field value from a Debian control-style paragraph, handling
/// continuation lines and values that start on the next line.
fn extract_field(text: &str, field: &str) -> Option<String> {
    let prefix = format!("{}:", field);
    let mut result = String::new();
    let mut in_field = false;

    for line in text.lines() {
        if line.starts_with(&prefix) {
            in_field = true;
            let rest = line.strip_prefix(&prefix).unwrap();
            let trimmed = rest.strip_prefix(' ').unwrap_or(rest).trim();
            if !trimmed.is_empty() {
                if !result.is_empty() {
                    result.push(' ');
                }
                result.push_str(trimmed);
            }
        } else if in_field && (line.starts_with(' ') || line.starts_with('\t')) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if !result.is_empty() {
                    result.push(' ');
                }
                result.push_str(trimmed);
            }
        } else if in_field {
            break;
        }
    }

    if result.trim().is_empty() {
        None
    } else {
        Some(result)
    }
}

pub fn parse_build_depends(control_path: &Path) -> Result<String> {
    let content = fs::read_to_string(control_path).context("failed to read debian/control")?;
    let first_para = content.split("\n\n").next().unwrap_or(&content);

    let mut parts = Vec::new();
    for field in ["Build-Depends", "Build-Depends-Arch", "Build-Depends-Indep"] {
        if let Some(v) = extract_field(first_para, field) {
            if !v.trim().is_empty() {
                parts.push(v);
            }
        }
    }

    Ok(parts.join(", "))
}

pub fn get_build_depends_from_apt(package: &str) -> Result<String> {
    let output = Command::new("apt-cache")
        .args(["showsrc", package])
        .output()
        .context("failed to run apt-cache showsrc")?;

    if !output.status.success() {
        bail!(
            "apt-cache showsrc failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    extract_field(&stdout, "Build-Depends")
        .with_context(|| format!("no Build-Depends found for source package '{}'", package))
}

/// Strip version constraints, architecture restrictions, build profiles, etc.
pub fn clean_deps(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '(' => {
                let mut depth = 1;
                while let Some(ch) = chars.next() {
                    if ch == '(' {
                        depth += 1;
                    } else if ch == ')' {
                        depth -= 1;
                    }
                    if depth == 0 {
                        break;
                    }
                }
            }
            '<' => {
                while let Some(ch) = chars.next() {
                    if ch == '>' {
                        break;
                    }
                }
            }
            '[' => {
                while let Some(ch) = chars.next() {
                    if ch == ']' {
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }

    let s = out.replace(":native", "");
    s.split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn collect_files_by_ext(dir: &Path, exts: &[&str]) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read dir: {:?}", dir))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if exts.iter().any(|ext| name.ends_with(ext)) {
            files.push(entry.path());
        }
    }
    Ok(files)
}
