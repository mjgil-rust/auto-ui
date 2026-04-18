mod display;

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Local;
use serde::Serialize;
use serde_json::Value;

pub use display::HeadlessDisplay;

pub type TraceFields = BTreeMap<String, String>;
pub const AUTO_UI_LAUNCH_BACKGROUND_ENV: &str = "AUTO_UI_LAUNCH_BACKGROUND";

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
}
