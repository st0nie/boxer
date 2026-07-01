use anyhow::{Context, Result};
use std::process::{Command, Stdio};

pub fn command_status(cmd: &mut Command) -> Result<i32> {
    let status = cmd.status().with_context(|| command_err(cmd))?;
    Ok(status.code().unwrap_or(1))
}

pub fn command(cmd: &mut Command) -> Result<()> {
    let code = command_status(cmd)?;
    if code != 0 {
        anyhow::bail!("command failed with exit code {}", code);
    }
    Ok(())
}

pub fn command_output(cmd: &mut Command) -> Result<std::process::Output> {
    let output = cmd.output().with_context(|| command_err(cmd))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{}: {}", command_err(cmd), stderr.trim());
    }
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

pub fn sudo(cmd: &mut Command, sudo_bin: &str) {
    let program = cmd.get_program().to_os_string();
    let args = cmd
        .get_args()
        .map(|a| a.to_os_string())
        .collect::<Vec<_>>();

    *cmd = Command::new(sudo_bin);
    cmd.arg(program);
    cmd.args(args);
}

pub fn new_command(program: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd
}

pub fn inherit_stdio(cmd: &mut Command) {
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
}

pub fn null_stdout(cmd: &mut Command) {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
}

pub fn run(program: &str, args: &[&str], needs_sudo: bool, sudo_bin: &str) -> Result<()> {
    let mut cmd = new_command(program, args);
    if needs_sudo {
        sudo(&mut cmd, sudo_bin);
    }
    inherit_stdio(&mut cmd);
    command(&mut cmd)
}

pub fn run_env(program: &str, args: &[&str], key: &str, value: &str) -> Result<()> {
    let mut cmd = new_command(program, args);
    cmd.env(key, value);
    inherit_stdio(&mut cmd);
    command(&mut cmd)
}

pub fn run_env_capture(program: &str, args: &[&str], key: &str, value: &str) -> Result<std::process::Output> {
    let mut cmd = new_command(program, args);
    cmd.env(key, value);
    let output = cmd.output().with_context(|| command_err(&cmd))?;
    Ok(output)
}

pub fn capture(program: &str, args: &[&str]) -> Result<std::process::Output> {
    let mut cmd = new_command(program, args);
    command_output(&mut cmd)
}

pub const SUDO_BIN: &str = "sudo";
