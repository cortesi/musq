//! Workspace maintenance commands for this repository.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Project maintenance tasks")]
/// Command-line interface for xtask.
struct Cli {
    #[command(subcommand)]
    /// Task to run.
    command: Task,
}

#[derive(Subcommand)]
/// Supported xtask subcommands.
enum Task {
    /// Run formatting and clippy fixes.
    Tidy,
    /// Run the test suite via nextest.
    Test,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = workspace_root()?;

    match cli.command {
        Task::Tidy => tidy(&root),
        Task::Test => test(&root),
    }
}

/// Resolve the workspace root directory.
fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .context("xtask must live under the workspace root")?;
    Ok(root.to_path_buf())
}

/// Run format and clippy fixes for the workspace.
fn tidy(root: &Path) -> Result<()> {
    let mut fmt = Command::new("cargo");
    fmt.current_dir(root)
        .arg("+nightly")
        .arg("fmt")
        .arg("--all")
        .arg("--")
        .arg("--config-path")
        .arg("./rustfmt-nightly.toml");
    run(&mut fmt)?;

    let mut clippy = Command::new("cargo");
    clippy
        .current_dir(root)
        .arg("clippy")
        .arg("-q")
        .arg("--fix")
        .arg("--all")
        .arg("--all-targets")
        .arg("--all-features")
        .arg("--allow-dirty")
        .arg("--tests")
        .arg("--examples");
    run(&mut clippy)?;

    Ok(())
}

/// Run the test suite via nextest.
fn test(root: &Path) -> Result<()> {
    let mut nextest = Command::new("cargo");
    nextest
        .current_dir(root)
        .arg("nextest")
        .arg("run")
        .arg("--all");
    run(&mut nextest)
}

/// Execute a command and surface failures with context.
fn run(cmd: &mut Command) -> Result<()> {
    let rendered = render_command(cmd);
    println!("Running: {rendered}");

    let status = cmd
        .status()
        .with_context(|| format!("failed to run {rendered}"))?;

    if !status.success() {
        bail!("command failed: {rendered}");
    }

    Ok(())
}

/// Render a command as a printable string.
fn render_command(cmd: &Command) -> String {
    let mut rendered = cmd.get_program().to_string_lossy().to_string();
    for arg in cmd.get_args() {
        rendered.push(' ');
        rendered.push_str(&arg.to_string_lossy());
    }
    rendered
}
