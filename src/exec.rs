use anyhow::{Context, Result, bail};
use log::debug;
use std::process::{Command, Output, Stdio};

pub const SUDO_BIN: &str = "sudo";

/// Run `cmd` and return its exit code, logging the command at debug level.
pub fn command_status(cmd: &mut Command) -> Result<i32> {
    debug!("running command: {:?}", cmd);
    let status = cmd.status().with_context(|| command_err(cmd))?;
    Ok(status.code().unwrap_or(1))
}

/// Run `cmd` inheriting stdio, bailing on non-zero exit.
pub fn command(cmd: &mut Command) -> Result<()> {
    let code = command_status(cmd)?;
    if code != 0 {
        bail!("{}: exit code {}", command_err(cmd), code);
    }
    Ok(())
}

/// Run `cmd` capturing its output, bailing on non-zero exit.
pub fn command_output(cmd: &mut Command) -> Result<Output> {
    debug!("running command: {:?}", cmd);
    let output = cmd.output().with_context(|| command_err(cmd))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{}: {}", command_err(cmd), stderr.trim());
    }
    Ok(output)
}

/// Run `cmd` capturing its output **without** checking the exit status.
///
/// Use this when you need to inspect a command's stdout/stderr even when it
/// is expected to fail (e.g. probing for a recoverable error condition).
pub fn output(cmd: &mut Command) -> Result<Output> {
    debug!("running command: {:?}", cmd);
    let output = cmd.output().with_context(|| command_err(cmd))?;
    Ok(output)
}

pub fn command_err(cmd: &Command) -> String {
    format!(
        "failed to run: {} {}",
        cmd.get_program().to_string_lossy(),
        cmd.get_args()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// Re-wrap `cmd` so it runs under `sudo`.
pub fn sudo(cmd: &mut Command) {
    let program = cmd.get_program().to_os_string();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_os_string()).collect();
    *cmd = Command::new(SUDO_BIN);
    cmd.arg(program).args(args);
}

pub fn inherit_stdio(cmd: &mut Command) {
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
}

/// Inherit stdio, run, and bail on non-zero exit.
pub fn run(cmd: &mut Command) -> Result<()> {
    inherit_stdio(cmd);
    command(cmd)
}

/// Run `cmd` under sudo, inheriting stdio.
pub fn run_sudo(cmd: &mut Command) -> Result<()> {
    sudo(cmd);
    run(cmd)
}
