use anyhow::{Context, Result, bail};
use log::debug;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

/// Run `cmd` inheriting stdio, bailing on non-zero exit.
pub fn command(cmd: &mut Command) -> Result<()> {
    debug!("running command: {:?}", cmd);
    let status = cmd.status().with_context(|| command_err(cmd))?;
    let code = status.code().unwrap_or(1);
    if code != 0 {
        bail!("{}: exit code {}", command_err(cmd), code);
    }
    Ok(())
}

/// Run `cmd` capturing its output, bailing on non-zero exit.
pub fn command_output(cmd: &mut Command) -> Result<Output> {
    let output = output(cmd)?;
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

/// boxer configuration, loaded from `$XDG_CONFIG_HOME/boxer/config.toml`
/// (default `~/.config/boxer/config.toml`).
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct Config {
    /// Privilege-escalation command (e.g. `"keyi"`); defaults to `sudo`.
    root_cmd: Option<String>,
}

/// Privilege-escalation command used to re-wrap privileged commands.
///
/// Resolution order: the `BOXER_ROOT_CMD` environment variable, then
/// `root_cmd` in the config file, then `sudo`.
pub fn root_cmd() -> String {
    if let Ok(cmd) = std::env::var("BOXER_ROOT_CMD")
        && !cmd.trim().is_empty()
    {
        return cmd;
    }
    if let Some(cmd) = config_root_cmd() {
        return cmd;
    }
    "sudo".to_string()
}

/// Read `root_cmd` from the boxer TOML config file, if present.
fn config_root_cmd() -> Option<String> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    let text = std::fs::read_to_string(base.join("boxer").join("config.toml")).ok()?;
    let config: Config = toml::from_str(&text).ok()?;
    let cmd = config.root_cmd?;
    let cmd = cmd.trim();
    if cmd.is_empty() {
        None
    } else {
        Some(cmd.to_string())
    }
}

/// Re-wrap `cmd` so it runs under the configured root command.
pub fn sudo(cmd: &mut Command) {
    let program = cmd.get_program().to_os_string();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_os_string()).collect();
    *cmd = Command::new(root_cmd());
    cmd.arg(program).args(args);
}

/// Inherit stdio, run, and bail on non-zero exit.
pub fn run(cmd: &mut Command) -> Result<()> {
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command(cmd)
}

/// Run `cmd` under the configured root command, inheriting stdio.
pub fn run_sudo(cmd: &mut Command) -> Result<()> {
    sudo(cmd);
    run(cmd)
}

/// Guard that keeps cached root credentials alive until dropped.
///
/// `sudo` caches credentials per TTY for a limited time (typically 15
/// minutes); dropping the guard stops the background refresh.
pub struct RootSession {
    keepalive: Option<std::process::Child>,
}

impl Drop for RootSession {
    fn drop(&mut self) {
        if let Some(mut child) = self.keepalive.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Prime root credentials up front so the user is prompted for a password
/// (if the root command requires one) at the start of the pipeline instead
/// of mid-build, then refresh them every minute in the background until the
/// returned session is dropped. Passwordless root commands (e.g. keyi) make
/// both steps silent no-ops.
pub fn acquire_root() -> Result<RootSession> {
    let root = root_cmd();
    let mut prime = Command::new(&root);
    prime.arg("true");
    run(&mut prime).with_context(|| format!("failed to obtain root credentials via {}", root))?;

    let keepalive = Command::new("sh")
        .arg("-c")
        .arg("while :; do \"$0\" true; sleep 60; done")
        .arg(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok(); // keepalive is best-effort; priming already succeeded
    Ok(RootSession { keepalive })
}
