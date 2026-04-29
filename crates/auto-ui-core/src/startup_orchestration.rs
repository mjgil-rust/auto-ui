//! Generic run orchestration for startup-driven execution modes.
//!
//! This module provides a unified orchestration layer that works with any
//! TargetAdapter implementation for startup-driven scenarios.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::{log_line, request_background_launch, CommandSpec, LaunchedRun, PreparedRun};

/// Configuration for startup-driven orchestration.
#[derive(Clone, Debug)]
pub struct StartupOrchestrationConfig {
    /// Maximum time to wait for process completion.
    pub timeout: Duration,
    /// Working directory for the process.
    pub working_dir: Option<PathBuf>,
    /// Whether to request background launch via env var.
    pub request_background_launch: bool,
    /// Progress log path for status updates.
    pub progress_log: Option<PathBuf>,
}

impl Default for StartupOrchestrationConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),
            working_dir: None,
            request_background_launch: true,
            progress_log: None,
        }
    }
}

impl StartupOrchestrationConfig {
    /// Sets the timeout for process completion.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the working directory.
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Sets the progress log path.
    pub fn with_progress_log(mut self, path: PathBuf) -> Self {
        self.progress_log = Some(path);
        self
    }
}

/// Orchestrator for startup-driven execution.
///
/// This struct handles launching and managing processes that drive themselves
/// after startup, without requiring harness window management.
#[derive(Default)]
pub struct StartupOrchestrator {
    config: StartupOrchestrationConfig,
}

impl StartupOrchestrator {
    /// Creates a new orchestrator with the given configuration.
    pub fn new(config: StartupOrchestrationConfig) -> Self {
        Self { config }
    }

    /// Creates a new orchestrator with default configuration.
    pub fn default_config() -> Self {
        Self::default()
    }

    /// Sets the timeout for process completion.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Sets the working directory.
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.config.working_dir = Some(dir);
        self
    }

    /// Sets the progress log path.
    pub fn with_progress_log(mut self, path: PathBuf) -> Self {
        self.config.progress_log = Some(path);
        self
    }

    /// Launches a prepared run and waits for completion.
    ///
    /// Returns the launched run handle on success, or an error if launch or
    /// waiting fails.
    pub fn launch_and_wait(&self, prepared: &PreparedRun) -> Result<LaunchedRun> {
        let command_spec = match &prepared.strategy {
            crate::LaunchStrategy::ManagedProcess { command, .. } => command,
            crate::LaunchStrategy::AutonomousProcess { command, .. } => command,
            crate::LaunchStrategy::ExistingWindow { .. } => {
                bail!("ExistingWindow strategy is not supported by StartupOrchestrator");
            }
        };

        self.launch_and_wait_with_command(command_spec)
    }

    /// Launches a command spec and waits for completion.
    pub fn launch_and_wait_with_command(&self, command_spec: &CommandSpec) -> Result<LaunchedRun> {
        let start = Instant::now();
        let progress_path = self.config.progress_log.clone();

        // Log the launch
        log_line(
            format!(
                "launching: {} {:?}",
                command_spec.program.display(),
                command_spec.args
            ),
            progress_path.as_deref(),
        )?;

        // Build the command
        let mut cmd = self.build_command(command_spec)?;

        // Spawn the process
        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn {}", command_spec.program.display()))?;

        let pid = child.id();

        log_line(
            format!("process spawned with PID: {}", pid),
            progress_path.as_deref(),
        )?;

        // Wait for completion with timeout
        let deadline = start + self.config.timeout;

        let exit_status = loop {
            // Check if we've exceeded the timeout
            let elapsed = start.elapsed();
            if elapsed >= self.config.timeout {
                // Kill the process and bail
                log_line(
                    format!("timeout after {:?}, killing process", elapsed),
                    progress_path.as_deref(),
                )?;
                child.kill()?;
                bail!("process timed out after {:?}", self.config.timeout);
            }

            // Try to wait non-blocking
            match child.try_wait()? {
                Some(status) => break status,
                None => {
                    // Small sleep to avoid busy-waiting
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        };

        let elapsed = start.elapsed();
        log_line(
            format!(
                "process exited after {:?} with status: {}",
                elapsed, exit_status
            ),
            progress_path.as_deref(),
        )?;

        if !exit_status.success() {
            bail!("process exited with non-zero status: {}", exit_status);
        }

        Ok(LaunchedRun {
            pid: Some(pid),
            window_id: None,
            command: command_spec.clone(),
            env: BTreeMap::new(),
        })
    }

    /// Builds a std::process::Command from a CommandSpec.
    fn build_command(&self, spec: &CommandSpec) -> Result<Command> {
        let mut cmd = Command::new(&spec.program);

        // Add arguments
        for arg in &spec.args {
            cmd.arg(arg);
        }

        // Set working directory
        if let Some(ref cwd) = spec.cwd {
            cmd.current_dir(cwd);
        } else if let Some(ref work_dir) = self.config.working_dir {
            cmd.current_dir(work_dir);
        }

        // Merge environment variables
        for (key, value) in &spec.env {
            cmd.env(key, value);
        }

        // Request background launch if configured
        if self.config.request_background_launch {
            request_background_launch(&mut cmd);
        }

        // Set up stdout/stderr
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        Ok(cmd)
    }
}

/// Runs a startup-driven scenario using the given adapter.
///
/// This is a convenience function that handles the full lifecycle:
/// 1. Prepare the run via the adapter
/// 2. Launch and wait via StartupOrchestrator
/// 3. Collect results via the adapter
pub fn run_startup_scenario(
    prepared: &PreparedRun,
    config: StartupOrchestrationConfig,
) -> Result<crate::CollectedData> {
    let orchestrator = StartupOrchestrator::new(config);
    let _launched = orchestrator.launch_and_wait(prepared)?;

    // For startup-driven scenarios, the adapter would collect artifacts
    // In a full implementation, this would call the adapter's collect method
    Ok(crate::CollectedData::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orchestration_config_default() {
        let config = StartupOrchestrationConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(300));
        assert!(config.working_dir.is_none());
        assert!(config.request_background_launch);
        assert!(config.progress_log.is_none());
    }

    #[test]
    fn orchestration_config_builder() {
        let config = StartupOrchestrationConfig::default()
            .with_timeout(Duration::from_secs(60))
            .with_working_dir(PathBuf::from("/tmp"))
            .with_progress_log(PathBuf::from("/tmp/progress.log"));

        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.working_dir, Some(PathBuf::from("/tmp")));
        assert_eq!(
            config.progress_log,
            Some(PathBuf::from("/tmp/progress.log"))
        );
    }

    #[test]
    fn command_spec_to_command() {
        let spec = CommandSpec::new("/bin/ls")
            .arg("-la")
            .arg("/tmp")
            .env("TEST_VAR", "test_value");

        let config = StartupOrchestrationConfig::default();
        let orchestrator = StartupOrchestrator::new(config);
        let cmd = orchestrator.build_command(&spec).unwrap();

        assert_eq!(cmd.get_program().to_string_lossy(), "/bin/ls");
    }

    #[test]
    fn launch_strategy_not_supported_for_existing_window() {
        let prepared = PreparedRun {
            strategy: crate::LaunchStrategy::ExistingWindow {
                selector: crate::WindowSelector::Active,
            },
            ..Default::default()
        };

        let config = StartupOrchestrationConfig::default();
        let orchestrator = StartupOrchestrator::new(config);
        let result = orchestrator.launch_and_wait(&prepared);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("ExistingWindow"));
        assert!(err.to_string().contains("not supported"));
    }

    #[test]
    fn startup_orchestrator_spawns_echo_command() {
        let spec = CommandSpec::new("/bin/echo").arg("hello world");

        let config = StartupOrchestrationConfig::default();
        let orchestrator = StartupOrchestrator::new(config);
        let result = orchestrator.launch_and_wait_with_command(&spec);

        assert!(result.is_ok());
        let run = result.unwrap();
        assert!(run.pid.is_some());
        assert!(run.window_id.is_none());
    }

    #[test]
    fn startup_orchestrator_respects_timeout() {
        let spec = CommandSpec::new("/bin/sleep").arg("10");

        let config = StartupOrchestrationConfig::default().with_timeout(Duration::from_millis(100));
        let orchestrator = StartupOrchestrator::new(config);
        let result = orchestrator.launch_and_wait_with_command(&spec);

        assert!(result.is_err());
        let err = result.unwrap_err();
        // The error message should mention timeout or timed out
        assert!(
            err.to_string().contains("timeout") || err.to_string().contains("timed out"),
            "expected timeout error, got: {}",
            err
        );
    }

    #[test]
    fn startup_orchestrator_returns_error_on_nonzero_exit() {
        // Use a command that exits with non-zero status
        let spec = CommandSpec::new("/bin/sh").arg("-c").arg("exit 1");

        let config = StartupOrchestrationConfig::default();
        let orchestrator = StartupOrchestrator::new(config);
        let result = orchestrator.launch_and_wait_with_command(&spec);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("non-zero status"));
    }

    #[test]
    fn startup_orchestrator_with_args_having_spaces() {
        let spec = CommandSpec::new("/bin/echo")
            .arg("hello")
            .arg("world with spaces");

        let config = StartupOrchestrationConfig::default();
        let orchestrator = StartupOrchestrator::new(config);
        let result = orchestrator.launch_and_wait_with_command(&spec);

        assert!(result.is_ok());
    }

    #[test]
    fn run_startup_scenario_convenience_function() {
        let prepared = PreparedRun {
            strategy: crate::LaunchStrategy::AutonomousProcess {
                command: CommandSpec::new("/bin/echo").arg("test"),
                background: false,
            },
            ..Default::default()
        };

        let config = StartupOrchestrationConfig::default();
        let result = run_startup_scenario(&prepared, config);

        assert!(result.is_ok());
    }
}
