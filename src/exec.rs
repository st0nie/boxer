use anyhow::{Context, Result, bail};
use log::{debug, warn};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::{OnceLock, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
/// `root_cmd` in the config file, then `sudo`. Cached after the first lookup.
pub fn root_cmd() -> &'static str {
    static ROOT_CMD: OnceLock<String> = OnceLock::new();
    ROOT_CMD.get_or_init(|| {
        let env_cmd = std::env::var("BOXER_ROOT_CMD").ok();
        resolve_root_cmd_from(env_cmd.as_deref(), config_root_cmd())
    })
}

fn resolve_root_cmd_from(env_cmd: Option<&str>, config_cmd: Option<String>) -> String {
    if let Some(cmd) = env_cmd.map(str::trim).filter(|c| !c.is_empty()) {
        return cmd.to_string();
    }
    if let Some(cmd) = config_cmd {
        let trimmed = cmd.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "sudo".to_string()
}

/// Read `root_cmd` from the boxer TOML config file, if present.
fn config_root_cmd() -> Option<String> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    let path = base.join("boxer").join("config.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            warn!("failed to read {}: {}", path.display(), e);
            return None;
        }
    };
    match parse_config_root_cmd(&text) {
        Ok(cmd) => cmd,
        Err(e) => {
            warn!("failed to parse {}: {}", path.display(), e);
            None
        }
    }
}

fn parse_config_root_cmd(text: &str) -> std::result::Result<Option<String>, toml::de::Error> {
    let config: Config = toml::from_str(text)?;
    Ok(config
        .root_cmd
        .map(|cmd| cmd.trim().to_string())
        .filter(|cmd| !cmd.is_empty()))
}

/// `true` when the current process is already running with effective UID 0.
#[cfg(unix)]
pub fn is_root() -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self")
        .map(|m| m.uid() == 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
pub fn is_root() -> bool {
    false
}

/// Re-wrap `cmd` so it runs under the configured root command, unless already
/// running as root.
pub fn sudo(cmd: &mut Command) {
    if is_root() {
        return;
    }
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
/// minutes); dropping the guard signals the background refresh thread to exit
/// immediately without leaving orphan child processes behind.
pub struct RootSession {
    stop_tx: Option<mpsc::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl RootSession {
    fn spawn_keepalive(root: String, interval: Duration) -> Self {
        let (stop_tx, stop_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            while let Err(mpsc::RecvTimeoutError::Timeout) = stop_rx.recv_timeout(interval) {
                let _ = Command::new(&root)
                    .arg("true")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        });
        Self {
            stop_tx: Some(stop_tx),
            worker: Some(worker),
        }
    }
}

impl Drop for RootSession {
    fn drop(&mut self) {
        drop(self.stop_tx.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Prime root credentials up front so the user is prompted for a password
/// (if the root command requires one) at the start of the pipeline instead
/// of mid-build, then refresh them every minute in a background thread until
/// the returned session is dropped. Passwordless root commands (e.g. `keyi`)
/// or running directly as root make this a silent fast path.
pub fn acquire_root() -> Result<RootSession> {
    if is_root() {
        return Ok(RootSession {
            stop_tx: None,
            worker: None,
        });
    }

    let root = root_cmd();
    let mut prime = Command::new(root);
    prime.arg("true");
    run(&mut prime).with_context(|| format!("failed to obtain root credentials via {}", root))?;

    Ok(RootSession::spawn_keepalive(
        root.to_string(),
        Duration::from_secs(60),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_root_cmd_in_priority_order() {
        assert_eq!(
            resolve_root_cmd_from(Some("keyi"), Some("doas".to_string())),
            "keyi"
        );
        assert_eq!(
            resolve_root_cmd_from(Some("  "), Some("doas".to_string())),
            "doas"
        );
        assert_eq!(resolve_root_cmd_from(None, Some("  ".to_string())), "sudo");
        assert_eq!(resolve_root_cmd_from(None, None), "sudo");
    }

    #[test]
    fn parses_config_toml_root_cmd() {
        assert_eq!(
            parse_config_root_cmd("root_cmd = \"keyi\"\n").unwrap(),
            Some("keyi".to_string())
        );
        assert_eq!(parse_config_root_cmd("root_cmd = \"  \"\n").unwrap(), None);
        assert_eq!(parse_config_root_cmd("").unwrap(), None);
        assert!(parse_config_root_cmd("root_cmd = [invalid").is_err());
    }

    #[test]
    fn root_session_drop_stops_worker_promptly() {
        let session = RootSession::spawn_keepalive("true".to_string(), Duration::from_secs(3600));
        drop(session);
    }
}
