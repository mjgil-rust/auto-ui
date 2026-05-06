use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use auto_ui_adapter_gpui as gpui;
use auto_ui_adapter_rust_chatbot as rust_chatbot;
use auto_ui_core::{normalize_name, parse_scenario_file};

#[derive(Debug, Args)]
pub struct ValidateScenarioArgs {
    /// Path to TOML scenario configuration file to validate
    #[arg(long)]
    pub config: PathBuf,
}

pub fn run(args: &ValidateScenarioArgs) -> Result<()> {
    let scenario_file = parse_scenario_file(&args.config)?;
    let target = normalize_name(&scenario_file.target);
    let scenario = normalize_name(&scenario_file.scenario);

    match target.as_str() {
        "rust_chatbot" => {
            rust_chatbot::validate_named_scenario(&scenario, &scenario_file.value)?;
        }
        "gpui_component_testing" => {
            gpui::validate_named_scenario(&scenario, &scenario_file.value)?;
        }
        other => {
            anyhow::bail!(
                "unknown target: {other}. Supported targets: rust_chatbot, gpui_component_testing"
            );
        }
    }

    println!(
        "valid: {} / {}",
        scenario_file.target, scenario_file.scenario
    );
    println!("  version: {}", scenario_file.version);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_ui_core::repo_root;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("auto-ui-{name}-{nanos}-{}", std::process::id()))
    }

    #[test]
    fn validate_command_runs_without_panic() {
        let path = repo_root().join("examples/rust-chatbot-debug.toml");
        let args = ValidateScenarioArgs { config: path };
        run(&args).unwrap();
    }

    #[test]
    fn validate_command_rejects_invalid_config() {
        let path = unique_temp_path("validate-invalid");
        fs::write(
            &path,
            r#"
version = "1"
target = "rust_chatbot"
scenario = "nonexistent"
[window]
widths = []
"#,
        )
        .unwrap();

        let args = ValidateScenarioArgs {
            config: path.clone(),
        };
        let result = run(&args);
        assert!(result.is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn validate_command_rejects_unknown_target() {
        let path = unique_temp_path("validate-unknown-target");
        fs::write(
            &path,
            r#"
version = "1"
target = "unknown_target"
scenario = "debug"
"#,
        )
        .unwrap();

        let args = ValidateScenarioArgs {
            config: path.clone(),
        };
        let result = run(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown target"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn validate_command_rejects_unknown_scenario() {
        let path = unique_temp_path("validate-unknown-scenario");
        fs::write(
            &path,
            r#"
version = "1"
target = "rust_chatbot"
scenario = "nonexistent_scenario"
"#,
        )
        .unwrap();

        let args = ValidateScenarioArgs {
            config: path.clone(),
        };
        let result = run(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported"));
        let _ = fs::remove_file(path);
    }
}
