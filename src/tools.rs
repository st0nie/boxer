use crate::Colors;
use crate::args::Args;
use crate::exec::run_sudo;
use crate::fmt::{detail, success, warning};

use anyhow::{Context, Result, bail};
use std::io::{BufRead, Write};
use std::process::Command;
use which::which;

/// External binaries boxer shells out to, paired with the Debian package that
/// provides them. `dpkg-buildpackage` and `dpkg-parsechangelog` both map to
/// `dpkg-dev`; package names are deduplicated before reaching `apt-get`.
const TOOL_PACKAGES: &[(&str, &str)] = &[
    ("quilt", "quilt"),
    ("mk-build-deps", "devscripts"),
    ("dpkg-buildpackage", "dpkg-dev"),
    ("dpkg-parsechangelog", "dpkg-dev"),
];

/// Ensure every binary in `bins` is reachable on `$PATH`, installing missing
/// ones via the configured root command (`BOXER_ROOT_CMD`, default `sudo`)
/// followed by `apt-get install -y`. Silent no-op when all are present;
/// prompts for confirmation unless `args.yes` is set.
pub fn ensure_installed(c: &Colors, args: &Args, bins: &[&str]) -> Result<()> {
    let mut missing: Vec<(&str, &str)> = Vec::new();
    for &bin in bins {
        if which(bin).is_err() {
            let pkg = TOOL_PACKAGES
                .iter()
                .find(|(b, _)| *b == bin)
                .map(|(_, p)| *p)
                .unwrap_or(bin);
            missing.push((bin, pkg));
        }
    }

    if missing.is_empty() {
        return Ok(());
    }

    warning(c, "missing required tools");
    for (bin, pkg) in &missing {
        detail(c, &format!("{} ({})", bin, pkg));
    }

    let mut packages: Vec<&str> = missing.iter().map(|(_, p)| *p).collect();
    packages.sort_unstable();
    packages.dedup();

    if !args.yes {
        print!("Install these packages via apt? [Y/n] ");
        std::io::stdout()
            .flush()
            .with_context(|| "failed to flush stdout")?;
        let mut input = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut input)
            .with_context(|| "failed to read prompt response")?;
        let confirmed = matches!(input.trim().to_lowercase().as_str(), "" | "y" | "yes");
        if !confirmed {
            bail!("required tools are missing; install them and retry");
        }
    }

    let mut cmd = Command::new("apt-get");
    cmd.args(["install", "-y"])
        .args(&packages)
        .env("DEBIAN_FRONTEND", "noninteractive");
    run_sudo(&mut cmd)?;

    success(c, "packages installed");
    Ok(())
}
