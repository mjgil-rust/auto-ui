mod adapter_lifecycle;
mod display;
mod adapter_registry;
mod startup_orchestration;
mod interactive_window_orchestration;
mod hybrid_orchestration;

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use display::HeadlessDisplay;

pub type TraceFields = BTreeMap<String, String>;
pub const AUTO_UI_LAUNCH_BACKGROUND_ENV: &str = "AUTO_UI_LAUNCH_BACKGROUND";

pub use adapter_registry::{AdapterRegistry, RegisteredAdapter};
pub use adapter_lifecycle::{
    run_lifecycle, run_lifecycle_collect, LifecycleError, LifecycleReporter, LifecycleResult,
};
pub use startup_orchestration::{run_startup_scenario, StartupOrchestrationConfig, StartupOrchestrator};
pub use interactive_window_orchestration::{
    InteractiveOrchestrationConfig, InteractiveWindowOrchestrator, WindowGeometry, WindowState,
};
pub use hybrid_orchestration::{HybridOrchestrationConfig, HybridOrchestrator};

pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug)]
pub struct ScenarioFile {
    pub target: String,
    pub scenario: String,
    pub output_dir: Option<String>,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub struct CompletedRun {
    pub output_dir: PathBuf,
    pub report_path: PathBuf,
}

/// Execution mode for a run, determining how the harness orchestrates the target.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Harness launches the target and waits for it to complete without user interaction.
    #[default]
    StartupDriven,
    /// Harness launches the target and manages an interactive window for user input.
    InteractiveWindow,
    /// Combination of startup-driven launch with interactive window management.
    Hybrid,
}

/// Status of a run execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Error,
}

impl Default for RunStatus {
    fn default() -> Self {
        RunStatus::Pending
    }
}

/// Request to execute a scenario run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RunRequest {
    /// Target identifier (e.g., "rust_chatbot", "gpui_component_testing").
    pub target: String,
    /// Scenario name within the target (e.g., "debug", "scroll_matrix").
    pub scenario: String,
    /// Execution mode for the harness orchestration.
    pub execution_mode: ExecutionMode,
    /// Optional override for the output directory.
    pub output_dir: Option<String>,
    /// Additional configuration as raw JSON value.
    pub config: Value,
}

impl RunRequest {
    /// Creates a new run request with the given target, scenario, and config.
    pub fn new(
        target: impl Into<String>,
        scenario: impl Into<String>,
        execution_mode: ExecutionMode,
        config: Value,
    ) -> Self {
        Self {
            target: target.into(),
            scenario: scenario.into(),
            execution_mode,
            output_dir: None,
            config,
        }
    }

    /// Sets the output directory override.
    pub fn with_output_dir(mut self, output_dir: impl Into<String>) -> Self {
        self.output_dir = Some(output_dir.into());
        self
    }
}

/// Result of a completed or failed run execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RunResult {
    /// Final status of the run.
    pub status: RunStatus,
    /// Output directory containing artifacts.
    pub output_dir: PathBuf,
    /// Path to the generated report file.
    pub report_path: PathBuf,
    /// Error message if the run failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RunResult {
    /// Creates a successful run result.
    pub fn completed(output_dir: PathBuf, report_path: PathBuf) -> Self {
        Self {
            status: RunStatus::Completed,
            output_dir,
            report_path,
            error: None,
        }
    }

    /// Creates a failed run result with an error message.
    pub fn error(output_dir: PathBuf, report_path: PathBuf, error: impl Into<String>) -> Self {
        Self {
            status: RunStatus::Error,
            output_dir,
            report_path,
            error: Some(error.into()),
        }
    }

    /// Returns true if the run completed successfully.
    pub fn is_success(&self) -> bool {
        self.status == RunStatus::Completed
    }
}

/// Specifies how to select a target window for automation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WindowSelector {
    /// Select by PID.
    Pid { value: u32 },
    /// Select by window title (exact match).
    Title { value: String },
    /// Select by window title pattern (regex).
    TitlePattern { value: String },
    /// Select the active/focused window.
    #[default]
    Active,
}

/// Source for trace/log data from the target application.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceSource {
    /// Trace from a file on disk.
    File {
        path: PathBuf,
        /// Pattern to match the latest trace file (e.g., "*.log.*").
        pattern: Option<String>,
    },
    /// Trace from an environment variable.
    EnvVar { name: String },
    /// No trace source for this run.
    None,
}

/// Source of artifacts to import after run completion.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub struct ImportSource {
    /// Artifact kind identifier.
    pub kind: String,
    /// Source path or glob pattern.
    pub source: String,
    /// Optional destination name within the output directory.
    pub dest: Option<String>,
}

/// Command specification for launching a process.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CommandSpec {
    /// Path to the executable.
    pub program: PathBuf,
    /// Command-line arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables to set (merged with existing env).
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Working directory for the command.
    pub cwd: Option<PathBuf>,
}

impl CommandSpec {
    /// Creates a new command spec with the given program.
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            ..Default::default()
        }
    }

    /// Adds an argument to the command.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Adds multiple arguments to the command.
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(|a| a.into()));
        self
    }

    /// Sets an environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Sets the working directory.
    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }
}

/// Strategy for launching and managing the target application.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LaunchStrategy {
    /// Attach to an existing window without launching a new process.
    ExistingWindow {
        selector: WindowSelector,
    },
    /// Launch and manage the process lifecycle (window management, cleanup).
    ManagedProcess {
        command: CommandSpec,
        /// Whether to request background launch via env var.
        background: bool,
    },
    /// Launch as an autonomous process without harness window management.
    AutonomousProcess {
        command: CommandSpec,
        /// Whether to request background launch via env var.
        background: bool,
    },
}

/// Prepared run containing all information needed to launch and track a scenario.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PreparedRun {
    /// How to launch the target application.
    pub strategy: LaunchStrategy,
    /// Expected window after launch (for verification).
    pub expected_window: Option<WindowSelector>,
    /// Source for trace/log data collection.
    pub trace_source: TraceSource,
    /// Artifacts to import after run completion.
    #[serde(default)]
    pub import_sources: Vec<ImportSource>,
}

impl Default for PreparedRun {
    fn default() -> Self {
        Self {
            strategy: LaunchStrategy::ManagedProcess {
                command: CommandSpec::default(),
                background: false,
            },
            expected_window: None,
            trace_source: TraceSource::None,
            import_sources: Vec::new(),
        }
    }
}

/// Context passed to adapters for run execution.
#[derive(Clone, Debug)]
pub struct AdapterContext {
    /// Root path of the target application.
    pub app_root: PathBuf,
    /// Output directory for this run.
    pub output_dir: PathBuf,
    /// Optional scenario-specific configuration.
    pub config: Value,
}

impl AdapterContext {
    /// Creates a new adapter context.
    pub fn new(app_root: PathBuf, output_dir: PathBuf, config: Value) -> Self {
        Self {
            app_root,
            output_dir,
            config,
        }
    }
}

/// Reference to a discoverable scenario.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ScenarioRef {
    /// Unique name of the scenario.
    pub name: String,
    /// Human-readable description.
    pub description: Option<String>,
}

/// Specification for running a scenario.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ScenarioSpec {
    /// Name of the scenario to run.
    pub name: String,
    /// Additional configuration for the scenario.
    #[serde(default)]
    pub config: Value,
}

/// Handle for a launched run.
#[derive(Clone, Debug)]
pub struct LaunchedRun {
    /// PID of the launched process, if applicable.
    pub pid: Option<u32>,
    /// Window handle, if applicable.
    pub window_id: Option<String>,
    /// Command that was launched.
    pub command: CommandSpec,
    /// Environment at launch time.
    pub env: BTreeMap<String, String>,
}

/// Result data collected after run completion.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CollectedData {
    /// Collected measurements.
    #[serde(default)]
    pub measurements: Vec<Measurement>,
    /// Collected events.
    #[serde(default)]
    pub events: Vec<Event>,
    /// Collected trace fields.
    #[serde(default)]
    pub trace_fields: TraceFields,
    /// Artifacts produced by the run.
    #[serde(default)]
    pub artifacts: Vec<ArtifactRef>,
}

/// A single measurement row.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Measurement {
    pub name: String,
    pub value: f64,
    pub unit: Option<String>,
}

/// A lifecycle event.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Event {
    pub timestamp: String,
    pub kind: String,
    pub message: Option<String>,
}

/// Reference to an artifact produced by a run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ArtifactRef {
    pub kind: String,
    pub path: String,
}

/// Core trait for target adapters.
///
/// Adapters implement this trait to provide target-specific logic for
/// scenario discovery, preparation, launching, data collection, and cleanup.
pub trait TargetAdapter: Send + Sync {
    /// Returns the unique identifier for this adapter (e.g., "rust_chatbot").
    fn id(&self) -> &'static str;

    /// Discovers available scenarios for this adapter.
    fn discover_scenarios(&self) -> Vec<ScenarioRef>;

    /// Validates that a scenario name is valid for this adapter.
    fn supports_scenario(&self, scenario: &str) -> bool {
        self.discover_scenarios()
            .iter()
            .any(|s| s.name == scenario)
    }

    /// Prepares a run for the given scenario.
    fn prepare(&self, ctx: &AdapterContext, spec: &ScenarioSpec) -> Result<PreparedRun>;

    /// Launches a prepared run.
    fn launch(&self, ctx: &AdapterContext, prepared: &PreparedRun) -> Result<LaunchedRun>;

    /// Collects data from a completed or failed run.
    fn collect(&self, ctx: &AdapterContext, run: &LaunchedRun) -> Result<CollectedData>;

    /// Stops a running scenario.
    fn stop(&self, ctx: &AdapterContext, run: &LaunchedRun) -> Result<()>;
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate inside workspace")
        .to_path_buf()
}

pub fn run_command(command: &mut Command, check: bool) -> Result<CommandOutput> {
    let rendered = render_command(command);
    let output = command
        .output()
        .with_context(|| format!("failed to run {rendered}"))?;
    decode_output(output, &rendered, check)
}

pub fn request_background_launch(command: &mut Command) {
    command.env(AUTO_UI_LAUNCH_BACKGROUND_ENV, "1");
}

pub fn render_command(command: &Command) -> String {
    let mut parts = Vec::new();
    parts.push(command.get_program().to_string_lossy().into_owned());
    for arg in command.get_args() {
        parts.push(arg.to_string_lossy().into_owned());
    }
    parts.join(" ")
}

pub fn log_line(message: impl AsRef<str>, progress_path: Option<&Path>) -> Result<()> {
    let message = message.as_ref();
    println!("{message}");
    std::io::stdout().flush().ok();
    if let Some(progress_path) = progress_path {
        let mut fh = OpenOptions::new()
            .create(true)
            .append(true)
            .open(progress_path)
            .with_context(|| format!("failed to open {}", progress_path.display()))?;
        writeln!(fh, "{message}")
            .with_context(|| format!("failed to write {}", progress_path.display()))?;
    }
    Ok(())
}

pub fn ensure_display(app_name: &str) -> Result<()> {
    if env::var_os("DISPLAY").is_none() {
        bail!("DISPLAY is not set. Run this from a desktop terminal in the same X session as {app_name}.");
    }
    Ok(())
}

pub fn expand_path(raw_path: &str) -> Result<PathBuf> {
    let path = if raw_path == "~" {
        home_dir()?
    } else if let Some(stripped) = raw_path.strip_prefix("~/") {
        home_dir()?.join(stripped)
    } else {
        PathBuf::from(raw_path)
    };

    if path.exists() {
        path.canonicalize()
            .with_context(|| format!("failed to resolve {}", path.display()))
    } else if path.is_absolute() {
        Ok(path)
    } else {
        env::current_dir()
            .with_context(|| "failed to resolve current directory".to_string())?
            .join(&path)
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", path.display()))
    }
}

pub fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set"))
}

pub fn parse_widths(raw: &str) -> Result<Vec<u32>> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<u32>()
                .with_context(|| format!("invalid width {part}"))
        })
        .collect()
}

pub fn build_output_dir(output_dir: Option<&str>, prefix: &str) -> Result<PathBuf> {
    let path = if let Some(output_dir) = output_dir {
        if output_dir.starts_with('~') || output_dir.starts_with('/') {
            expand_path(output_dir)?
        } else {
            repo_root().join(output_dir)
        }
    } else {
        repo_root()
            .join("tmp")
            .join(format!("{prefix}-{}", Local::now().format("%Y%m%d-%H%M%S")))
    };
    fs::create_dir_all(&path).with_context(|| format!("failed to create {}", path.display()))?;
    path.canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))
}

pub fn write_json_file(path: &Path, value: &impl Serialize) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

pub fn parse_scenario_file(path: &Path) -> Result<ScenarioFile> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let toml_value: toml::Value =
        toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))?;
    let json_value = serde_json::to_value(toml_value)?;
    let target = json_value
        .get("target")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{} is missing a string target", path.display()))?
        .to_string();
    let scenario = json_value
        .get("scenario")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{} is missing a string scenario", path.display()))?
        .to_string();
    let output_dir = json_value
        .get("output_dir")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    Ok(ScenarioFile {
        target,
        scenario,
        output_dir,
        value: json_value,
    })
}

pub fn normalize_name(value: &str) -> String {
    value.replace('-', "_")
}

fn decode_output(output: Output, rendered: &str, check: bool) -> Result<CommandOutput> {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if check && !output.status.success() {
        bail!(
            "command failed: {rendered}\nstdout:\n{}\nstderr:\n{}",
            stdout.trim_end(),
            stderr.trim_end()
        );
    }
    Ok(CommandOutput { stdout, stderr })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("auto-ui-{name}-{nanos}-{}", std::process::id()))
    }

    #[test]
    fn parse_scenario_file_reads_core_fields() {
        let path = unique_temp_path("scenario");
        fs::write(
            &path,
            r#"
target = "rust_chatbot"
scenario = "debug"
output_dir = "tmp/example"
[app]
provider = "codex"
"#,
        )
        .unwrap();

        let scenario = parse_scenario_file(&path).unwrap();
        assert_eq!(scenario.target, "rust_chatbot");
        assert_eq!(scenario.scenario, "debug");
        assert_eq!(scenario.output_dir.as_deref(), Some("tmp/example"));
        assert_eq!(
            scenario
                .value
                .get("app")
                .and_then(|value| value.get("provider"))
                .and_then(Value::as_str),
            Some("codex")
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_scenario_file_requires_string_target() {
        let path = unique_temp_path("missing-target");
        fs::write(&path, "scenario = \"debug\"\ntarget = 7\n").unwrap();

        let err = parse_scenario_file(&path).unwrap_err().to_string();
        assert!(err.contains("missing a string target"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_scenario_file_requires_string_scenario() {
        let path = unique_temp_path("missing-scenario");
        fs::write(&path, "target = \"rust_chatbot\"\nscenario = false\n").unwrap();

        let err = parse_scenario_file(&path).unwrap_err().to_string();
        assert!(err.contains("missing a string scenario"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_widths_empty_string() {
        let result = parse_widths("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_widths_single_value() {
        let result = parse_widths("800").unwrap();
        assert_eq!(result, vec![800]);
    }

    #[test]
    fn parse_widths_multiple_values() {
        let result = parse_widths("800, 1024, 1280").unwrap();
        assert_eq!(result, vec![800, 1024, 1280]);
    }

    #[test]
    fn parse_widths_with_whitespace() {
        let result = parse_widths(" 800 , 1024 , 1280 ").unwrap();
        assert_eq!(result, vec![800, 1024, 1280]);
    }

    #[test]
    fn parse_widths_invalid_value() {
        let result = parse_widths("800,abc,1280");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid width"));
    }

    // Tests for shared run types (Task #4)

    #[test]
    fn execution_mode_default_is_startup_driven() {
        assert_eq!(ExecutionMode::default(), ExecutionMode::StartupDriven);
    }

    #[test]
    fn execution_mode_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&ExecutionMode::StartupDriven).unwrap(),
            "\"startup_driven\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutionMode::InteractiveWindow).unwrap(),
            "\"interactive_window\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutionMode::Hybrid).unwrap(),
            "\"hybrid\""
        );
    }

    #[test]
    fn execution_mode_deserializes_from_snake_case() {
        assert_eq!(
            serde_json::from_str::<ExecutionMode>("\"startup_driven\"").unwrap(),
            ExecutionMode::StartupDriven
        );
        assert_eq!(
            serde_json::from_str::<ExecutionMode>("\"interactive_window\"").unwrap(),
            ExecutionMode::InteractiveWindow
        );
        assert_eq!(
            serde_json::from_str::<ExecutionMode>("\"hybrid\"").unwrap(),
            ExecutionMode::Hybrid
        );
    }

    #[test]
    fn run_status_default_is_pending() {
        assert_eq!(RunStatus::default(), RunStatus::Pending);
    }

    #[test]
    fn run_status_serializes_correctly() {
        assert_eq!(serde_json::to_string(&RunStatus::Pending).unwrap(), "\"pending\"");
        assert_eq!(
            serde_json::to_string(&RunStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&RunStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&RunStatus::Error).unwrap(),
            "\"error\""
        );
    }

    #[test]
    fn run_status_deserializes_correctly() {
        assert_eq!(
            serde_json::from_str::<RunStatus>("\"pending\"").unwrap(),
            RunStatus::Pending
        );
        assert_eq!(
            serde_json::from_str::<RunStatus>("\"running\"").unwrap(),
            RunStatus::Running
        );
        assert_eq!(
            serde_json::from_str::<RunStatus>("\"completed\"").unwrap(),
            RunStatus::Completed
        );
        assert_eq!(
            serde_json::from_str::<RunStatus>("\"error\"").unwrap(),
            RunStatus::Error
        );
    }

    #[test]
    fn run_request_builder_pattern() {
        let config = serde_json::json!({"provider": "claude", "widths": [800, 1200]});
        let request = RunRequest::new("rust_chatbot", "debug", ExecutionMode::StartupDriven, config.clone())
            .with_output_dir("tmp/output");

        assert_eq!(request.target, "rust_chatbot");
        assert_eq!(request.scenario, "debug");
        assert_eq!(request.execution_mode, ExecutionMode::StartupDriven);
        assert_eq!(request.output_dir.as_deref(), Some("tmp/output"));
        assert_eq!(request.config, config);
    }

    #[test]
    fn run_request_default_execution_mode() {
        let request = RunRequest::new(
            "gpui_component_testing",
            "scroll_matrix",
            ExecutionMode::default(),
            serde_json::json!({}),
        );
        assert_eq!(request.execution_mode, ExecutionMode::StartupDriven);
    }

    #[test]
    fn run_request_serialization() {
        let request = RunRequest::new(
            "rust_chatbot",
            "header_debug",
            ExecutionMode::InteractiveWindow,
            serde_json::json!({"height": 600}),
        )
        .with_output_dir("tmp/results");

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: RunRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.target, request.target);
        assert_eq!(deserialized.scenario, request.scenario);
        assert_eq!(deserialized.execution_mode, request.execution_mode);
        assert_eq!(deserialized.output_dir, request.output_dir);
    }

    #[test]
    fn run_result_completed() {
        let output_dir = PathBuf::from("/tmp/test-output");
        let report_path = PathBuf::from("/tmp/test-output/report.json");
        let result = RunResult::completed(output_dir.clone(), report_path.clone());

        assert_eq!(result.status, RunStatus::Completed);
        assert_eq!(result.output_dir, output_dir);
        assert_eq!(result.report_path, report_path);
        assert!(result.error.is_none());
        assert!(result.is_success());
    }

    #[test]
    fn run_result_error() {
        let output_dir = PathBuf::from("/tmp/test-output");
        let report_path = PathBuf::from("/tmp/test-output/report.json");
        let result = RunResult::error(output_dir.clone(), report_path.clone(), "process exited early");

        assert_eq!(result.status, RunStatus::Error);
        assert_eq!(result.output_dir, output_dir);
        assert_eq!(result.report_path, report_path);
        assert_eq!(result.error.as_deref(), Some("process exited early"));
        assert!(!result.is_success());
    }

    #[test]
    fn run_result_error_omits_none_field() {
        let result = RunResult::completed(PathBuf::from("/tmp"), PathBuf::from("/tmp/report.json"));
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("error"));
    }

    #[test]
    fn run_result_error_includes_error_field() {
        let result = RunResult::error(PathBuf::from("/tmp"), PathBuf::from("/tmp/report.json"), "test error");
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("test error"));
    }

    #[test]
    fn run_result_serialization_roundtrip() {
        let result = RunResult::error(
            PathBuf::from("/tmp/output"),
            PathBuf::from("/tmp/output/report.json"),
            "something went wrong",
        );

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: RunResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.status, RunStatus::Error);
        assert_eq!(deserialized.error.as_deref(), Some("something went wrong"));
        assert!(!deserialized.is_success());
    }

    // Tests for Task #7: PreparedRun and related types

    #[test]
    fn window_selector_default_is_active() {
        assert_eq!(WindowSelector::default(), WindowSelector::Active);
    }

    #[test]
    fn window_selector_serializes_correctly() {
        assert_eq!(
            serde_json::to_string(&WindowSelector::Active).unwrap(),
            r#"{"type":"active"}"#
        );
        assert_eq!(
            serde_json::to_string(&WindowSelector::Pid { value: 12345 }).unwrap(),
            r#"{"type":"pid","value":12345}"#
        );
        assert_eq!(
            serde_json::to_string(&WindowSelector::Title { value: "test".to_string() }).unwrap(),
            r#"{"type":"title","value":"test"}"#
        );
        assert_eq!(
            serde_json::to_string(&WindowSelector::TitlePattern { value: ".*test.*".to_string() }).unwrap(),
            r#"{"type":"title_pattern","value":".*test.*"}"#
        );
    }

    #[test]
    fn window_selector_deserializes_correctly() {
        assert_eq!(
            serde_json::from_str::<WindowSelector>(r#"{"type":"active"}"#).unwrap(),
            WindowSelector::Active
        );
        assert_eq!(
            serde_json::from_str::<WindowSelector>(r#"{"type":"pid","value":12345}"#).unwrap(),
            WindowSelector::Pid { value: 12345 }
        );
        assert_eq!(
            serde_json::from_str::<WindowSelector>(r#"{"type":"title","value":"mywindow"}"#).unwrap(),
            WindowSelector::Title { value: "mywindow".to_string() }
        );
        assert_eq!(
            serde_json::from_str::<WindowSelector>(r#"{"type":"title_pattern","value":".*"}"#).unwrap(),
            WindowSelector::TitlePattern { value: ".*".to_string() }
        );
    }

    #[test]
    fn trace_source_serializes_correctly() {
        let none = TraceSource::None;
        assert_eq!(serde_json::to_string(&none).unwrap(), r#"{"type":"none"}"#);

        let file = TraceSource::File {
            path: PathBuf::from("/var/log/app.log"),
            pattern: Some("*.log.*".to_string()),
        };
        let json = serde_json::to_string(&file).unwrap();
        assert!(json.contains("\"type\":\"file\""));
        assert!(json.contains("\"/var/log/app.log\""));
        assert!(json.contains("\"*.log.*\""));

        let env_var = TraceSource::EnvVar { name: "TRACE_FILE".to_string() };
        assert_eq!(
            serde_json::to_string(&env_var).unwrap(),
            r#"{"type":"env_var","name":"TRACE_FILE"}"#
        );
    }

    #[test]
    fn trace_source_deserializes_correctly() {
        assert_eq!(
            serde_json::from_str::<TraceSource>(r#"{"type":"none"}"#).unwrap(),
            TraceSource::None
        );
        let file: TraceSource = serde_json::from_str(
            r#"{"type":"file","path":"/tmp/trace.log","pattern":"*.log.*"}"#
        ).unwrap();
        match file {
            TraceSource::File { path, pattern } => {
                assert_eq!(path, PathBuf::from("/tmp/trace.log"));
                assert_eq!(pattern, Some("*.log.*".to_string()));
            }
            _ => panic!("expected File variant"),
        }
        assert_eq!(
            serde_json::from_str::<TraceSource>(r#"{"type":"env_var","name":"MY_TRACE"}"#).unwrap(),
            TraceSource::EnvVar { name: "MY_TRACE".to_string() }
        );
    }

    #[test]
    fn import_source_serializes_correctly() {
        let source = ImportSource {
            kind: "csv".to_string(),
            source: "/tmp/results.csv".to_string(),
            dest: Some("data.csv".to_string()),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("\"kind\":\"csv\""));
        assert!(json.contains("\"/tmp/results.csv\""));
        assert!(json.contains("\"data.csv\""));
    }

    #[test]
    fn import_source_deserializes_correctly() {
        let json = r#"{"type":"import_source","kind":"log","source":"*.log","dest":null}"#;
        let source: ImportSource = serde_json::from_str(json).unwrap();
        assert_eq!(source.kind, "log");
        assert_eq!(source.source, "*.log");
        assert_eq!(source.dest, None);
    }

    #[test]
    fn command_spec_builder_pattern() {
        let cmd = CommandSpec::new("/usr/bin/app")
            .arg("--flag")
            .args(["--config", "/tmp/config"])
            .env("HOME", "/tmp")
            .env("DEBUG", "1")
            .cwd("/tmp/workdir");

        assert_eq!(cmd.program, PathBuf::from("/usr/bin/app"));
        assert_eq!(cmd.args, vec!["--flag", "--config", "/tmp/config"]);
        assert_eq!(cmd.env.get("HOME"), Some(&"/tmp".to_string()));
        assert_eq!(cmd.env.get("DEBUG"), Some(&"1".to_string()));
        assert_eq!(cmd.cwd, Some(PathBuf::from("/tmp/workdir")));
    }

    #[test]
    fn command_spec_default_is_empty() {
        let cmd = CommandSpec::default();
        assert_eq!(cmd.program, PathBuf::new());
        assert!(cmd.args.is_empty());
        assert!(cmd.env.is_empty());
        assert!(cmd.cwd.is_none());
    }

    #[test]
    fn command_spec_serialization() {
        let cmd = CommandSpec::new("/bin/ls").arg("-la");
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: CommandSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.program, PathBuf::from("/bin/ls"));
        assert_eq!(deserialized.args, vec!["-la"]);
    }

    #[test]
    fn launch_strategy_serializes_correctly() {
        let existing = LaunchStrategy::ExistingWindow {
            selector: WindowSelector::Title { value: "test".to_string() },
        };
        let json = serde_json::to_string(&existing).unwrap();
        assert!(json.contains("\"type\":\"existing_window\""));
        assert!(json.contains("\"title\""));

        let managed = LaunchStrategy::ManagedProcess {
            command: CommandSpec::new("/bin/app").arg("--run"),
            background: true,
        };
        let json = serde_json::to_string(&managed).unwrap();
        assert!(json.contains("\"type\":\"managed_process\""));
        assert!(json.contains("\"background\":true"));

        let autonomous = LaunchStrategy::AutonomousProcess {
            command: CommandSpec::new("/bin/daemon"),
            background: false,
        };
        let json = serde_json::to_string(&autonomous).unwrap();
        assert!(json.contains("\"type\":\"autonomous_process\""));
        assert!(json.contains("\"background\":false"));
    }

    #[test]
    fn launch_strategy_deserializes_correctly() {
        let existing: LaunchStrategy = serde_json::from_str(
            r#"{"type":"existing_window","selector":{"type":"active"}}"#
        ).unwrap();
        match existing {
            LaunchStrategy::ExistingWindow { selector } => {
                assert_eq!(selector, WindowSelector::Active);
            }
            _ => panic!("expected ExistingWindow variant"),
        }

        let managed: LaunchStrategy = serde_json::from_str(
            r#"{"type":"managed_process","command":{"program":"/bin/ls","args":["-l"]},"background":true}"#
        ).unwrap();
        match managed {
            LaunchStrategy::ManagedProcess { command, background } => {
                assert_eq!(command.program, PathBuf::from("/bin/ls"));
                assert!(background);
            }
            _ => panic!("expected ManagedProcess variant"),
        }

        let autonomous: LaunchStrategy = serde_json::from_str(
            r#"{"type":"autonomous_process","command":{"program":"/bin/daemon"},"background":false}"#
        ).unwrap();
        match autonomous {
            LaunchStrategy::AutonomousProcess { command, background } => {
                assert_eq!(command.program, PathBuf::from("/bin/daemon"));
                assert!(!background);
            }
            _ => panic!("expected AutonomousProcess variant"),
        }
    }

    #[test]
    fn prepared_run_default() {
        let run = PreparedRun::default();
        assert!(matches!(
            run.strategy,
            LaunchStrategy::ManagedProcess { .. }
        ));
        assert!(run.expected_window.is_none());
        assert!(matches!(run.trace_source, TraceSource::None));
        assert!(run.import_sources.is_empty());
    }

    #[test]
    fn prepared_run_serialization() {
        let run = PreparedRun {
            strategy: LaunchStrategy::AutonomousProcess {
                command: CommandSpec::new("/bin/test"),
                background: true,
            },
            expected_window: Some(WindowSelector::Pid { value: 1000 }),
            trace_source: TraceSource::File {
                path: PathBuf::from("/var/log/test.log"),
                pattern: None,
            },
            import_sources: vec![
                ImportSource {
                    kind: "csv".to_string(),
                    source: "*.csv".to_string(),
                    dest: None,
                },
            ],
        };

        let json = serde_json::to_string(&run).unwrap();
        let deserialized: PreparedRun = serde_json::from_str(&json).unwrap();

        assert!(matches!(
            deserialized.strategy,
            LaunchStrategy::AutonomousProcess { .. }
        ));
        assert!(matches!(
            deserialized.expected_window,
            Some(WindowSelector::Pid { value: 1000 })
        ));
        assert!(matches!(
            deserialized.trace_source,
            TraceSource::File { .. }
        ));
        assert_eq!(deserialized.import_sources.len(), 1);
    }

    // Tests for Task #5: TargetAdapter trait and related types

    #[test]
    fn adapter_context_creation() {
        let ctx = AdapterContext::new(
            PathBuf::from("/app/root"),
            PathBuf::from("/output/dir"),
            serde_json::json!({"key": "value"}),
        );
        assert_eq!(ctx.app_root, PathBuf::from("/app/root"));
        assert_eq!(ctx.output_dir, PathBuf::from("/output/dir"));
        assert_eq!(ctx.config, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn scenario_ref_serialization() {
        let reference = ScenarioRef {
            name: "test_scenario".to_string(),
            description: Some("A test scenario".to_string()),
        };
        let json = serde_json::to_string(&reference).unwrap();
        assert!(json.contains("\"name\":\"test_scenario\""));
        assert!(json.contains("\"description\":\"A test scenario\""));
    }

    #[test]
    fn scenario_spec_serialization() {
        let spec = ScenarioSpec {
            name: "my_scenario".to_string(),
            config: serde_json::json!({"timeout": 30}),
        };
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"name\":\"my_scenario\""));
        assert!(json.contains("\"timeout\":30"));
    }

    #[test]
    fn launched_run_creation() {
        let run = LaunchedRun {
            pid: Some(12345),
            window_id: Some("0x12345678".to_string()),
            command: CommandSpec::new("/bin/app").arg("--run"),
            env: BTreeMap::new(),
        };
        assert_eq!(run.pid, Some(12345));
        assert_eq!(run.window_id, Some("0x12345678".to_string()));
    }

    #[test]
    fn collected_data_default() {
        let data = CollectedData::default();
        assert!(data.measurements.is_empty());
        assert!(data.events.is_empty());
        assert!(data.trace_fields.is_empty());
        assert!(data.artifacts.is_empty());
    }

    #[test]
    fn collected_data_serialization() {
        let data = CollectedData {
            measurements: vec![Measurement {
                name: "duration_ms".to_string(),
                value: 1500.0,
                unit: Some("ms".to_string()),
            }],
            events: vec![Event {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                kind: "launch".to_string(),
                message: Some("Process started".to_string()),
            }],
            trace_fields: TraceFields::new(),
            artifacts: vec![ArtifactRef {
                kind: "screenshot".to_string(),
                path: "/output/screenshot.png".to_string(),
            }],
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"measurements\""));
        assert!(json.contains("\"events\""));
        assert!(json.contains("\"duration_ms\""));
        assert!(json.contains("\"screenshot\""));
    }

    #[test]
    fn measurement_serialization() {
        let m = Measurement {
            name: "bytes_per_second".to_string(),
            value: 1024.5,
            unit: Some("B/s".to_string()),
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"name\":\"bytes_per_second\""));
        assert!(json.contains("\"value\":1024.5"));
    }

    #[test]
    fn event_serialization() {
        let e = Event {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            kind: "error".to_string(),
            message: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"timestamp\":\"2024-01-01T00:00:00Z\""));
        assert!(json.contains("\"kind\":\"error\""));
    }

    #[test]
    fn artifact_ref_serialization() {
        let a = ArtifactRef {
            kind: "csv".to_string(),
            path: "/data/results.csv".to_string(),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"kind\":\"csv\""));
        assert!(json.contains("\"/data/results.csv\""));
    }

    // Fake adapter for testing the TargetAdapter trait

    struct FakeAdapter {
        target_id: &'static str,
        scenarios: Vec<ScenarioRef>,
    }

    impl FakeAdapter {
        fn new() -> Self {
            Self {
                target_id: "fake_adapter",
                scenarios: vec![
                    ScenarioRef {
                        name: "scenario_a".to_string(),
                        description: Some("Scenario A".to_string()),
                    },
                    ScenarioRef {
                        name: "scenario_b".to_string(),
                        description: None,
                    },
                ],
            }
        }
    }

    impl TargetAdapter for FakeAdapter {
        fn id(&self) -> &'static str {
            self.target_id
        }

        fn discover_scenarios(&self) -> Vec<ScenarioRef> {
            self.scenarios.clone()
        }

        fn prepare(
            &self,
            ctx: &AdapterContext,
            spec: &ScenarioSpec,
        ) -> Result<PreparedRun> {
            Ok(PreparedRun {
                strategy: LaunchStrategy::ManagedProcess {
                    command: CommandSpec::new(ctx.app_root.join("bin/target")),
                    background: true,
                },
                expected_window: None,
                trace_source: TraceSource::None,
                import_sources: vec![],
            })
        }

        fn launch(
            &self,
            ctx: &AdapterContext,
            prepared: &PreparedRun,
        ) -> Result<LaunchedRun> {
            Ok(LaunchedRun {
                pid: Some(42),
                window_id: None,
                command: prepared.strategy.default_command(),
                env: BTreeMap::new(),
            })
        }

        fn collect(
            &self,
            ctx: &AdapterContext,
            run: &LaunchedRun,
        ) -> Result<CollectedData> {
            Ok(CollectedData::default())
        }

        fn stop(&self, ctx: &AdapterContext, run: &LaunchedRun) -> Result<()> {
            Ok(())
        }
    }

    impl LaunchStrategy {
        fn default_command(&self) -> CommandSpec {
            match self {
                LaunchStrategy::ExistingWindow { .. } => CommandSpec::default(),
                LaunchStrategy::ManagedProcess { command, .. } => command.clone(),
                LaunchStrategy::AutonomousProcess { command, .. } => command.clone(),
            }
        }
    }

    #[test]
    fn target_adapter_trait_id() {
        let adapter = FakeAdapter::new();
        assert_eq!(adapter.id(), "fake_adapter");
    }

    #[test]
    fn target_adapter_discover_scenarios() {
        let adapter = FakeAdapter::new();
        let scenarios = adapter.discover_scenarios();
        assert_eq!(scenarios.len(), 2);
        assert!(scenarios.iter().any(|s| s.name == "scenario_a"));
        assert!(scenarios.iter().any(|s| s.name == "scenario_b"));
    }

    #[test]
    fn target_adapter_supports_scenario() {
        let adapter = FakeAdapter::new();
        assert!(adapter.supports_scenario("scenario_a"));
        assert!(adapter.supports_scenario("scenario_b"));
        assert!(!adapter.supports_scenario("unknown_scenario"));
    }

    #[test]
    fn target_adapter_prepare() {
        let adapter = FakeAdapter::new();
        let ctx = AdapterContext::new(
            PathBuf::from("/test/app"),
            PathBuf::from("/test/output"),
            serde_json::json!({}),
        );
        let spec = ScenarioSpec {
            name: "scenario_a".to_string(),
            config: serde_json::json!({}),
        };

        let prepared = adapter.prepare(&ctx, &spec).unwrap();
        assert!(matches!(
            prepared.strategy,
            LaunchStrategy::ManagedProcess { .. }
        ));
    }

    #[test]
    fn target_adapter_launch() {
        let adapter = FakeAdapter::new();
        let ctx = AdapterContext::new(
            PathBuf::from("/test/app"),
            PathBuf::from("/test/output"),
            serde_json::json!({}),
        );
        let prepared = PreparedRun::default();

        let launched = adapter.launch(&ctx, &prepared).unwrap();
        assert_eq!(launched.pid, Some(42));
    }

    #[test]
    fn target_adapter_collect() {
        let adapter = FakeAdapter::new();
        let ctx = AdapterContext::new(
            PathBuf::from("/test/app"),
            PathBuf::from("/test/output"),
            serde_json::json!({}),
        );
        let run = LaunchedRun {
            pid: Some(42),
            window_id: None,
            command: CommandSpec::default(),
            env: BTreeMap::new(),
        };

        let collected = adapter.collect(&ctx, &run).unwrap();
        assert!(collected.measurements.is_empty());
        assert!(collected.events.is_empty());
    }

    #[test]
    fn target_adapter_stop() {
        let adapter = FakeAdapter::new();
        let ctx = AdapterContext::new(
            PathBuf::from("/test/app"),
            PathBuf::from("/test/output"),
            serde_json::json!({}),
        );
        let run = LaunchedRun {
            pid: Some(42),
            window_id: None,
            command: CommandSpec::default(),
            env: BTreeMap::new(),
        };

        let result = adapter.stop(&ctx, &run);
        assert!(result.is_ok());
    }

    #[test]
    fn target_adapter_lifecycle() {
        // Integration test covering the full lifecycle
        let adapter = FakeAdapter::new();
        let ctx = AdapterContext::new(
            PathBuf::from("/test/app"),
            PathBuf::from("/test/output"),
            serde_json::json!({}),
        );
        let spec = ScenarioSpec {
            name: "scenario_a".to_string(),
            config: serde_json::json!({}),
        };

        // prepare
        let prepared = adapter.prepare(&ctx, &spec).unwrap();

        // launch
        let launched = adapter.launch(&ctx, &prepared).unwrap();
        assert!(launched.pid.is_some());

        // collect
        let collected = adapter.collect(&ctx, &launched).unwrap();
        assert!(collected.trace_fields.is_empty());

        // stop
        let result = adapter.stop(&ctx, &launched);
        assert!(result.is_ok());
    }
}
