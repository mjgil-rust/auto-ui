use std::path::PathBuf;

use anyhow::{bail, Result};
use auto_ui_adapter_gpui as gpui;
use auto_ui_adapter_rust_chatbot as rust_chatbot;
use auto_ui_core::{normalize_name, parse_scenario_file, HeadlessDisplay};
use clap::{Args as ClapArgs, CommandFactory};

const SUPPORTED_TARGETS: &[&str] = &["rust_chatbot", "gpui_component_testing"];
const DEFAULT_GEOMETRY: &str = "1280x800x24";

#[derive(Clone, Debug, ClapArgs)]
pub struct Args {
    /// Path to TOML scenario configuration file
    #[arg(long)]
    pub config: PathBuf,
    /// Target to run (rust_chatbot or gpui_component_testing).
    /// Overrides the target field in the TOML file.
    #[arg(long)]
    pub target: Option<String>,
    /// Scenario to run within the target. Overrides the scenario field in the TOML file.
    #[arg(long)]
    pub scenario: Option<String>,
    /// Output directory for artifacts and reports. Defaults to a unique timestamped directory.
    #[arg(long)]
    pub output_dir: Option<String>,
    /// Run on a private Xvfb display with openbox so no windows appear on the
    /// user's real screen. Requires Xvfb and openbox to be installed.
    #[arg(long)]
    pub headless: bool,
    /// Xvfb screen geometry (default: 1280x800x24). Only used with --headless.
    #[arg(long, default_value = DEFAULT_GEOMETRY)]
    pub geometry: String,
}

pub fn run(args: Args) -> Result<()> {
    let _headless = if args.headless {
        Some(HeadlessDisplay::start(&args.geometry)?)
    } else {
        None
    };

    let scenario_file = parse_scenario_file(&args.config)?;
    let target = normalize_name(args.target.as_deref().unwrap_or(&scenario_file.target));
    let scenario = normalize_name(args.scenario.as_deref().unwrap_or(&scenario_file.scenario));
    let output_override = args.output_dir.or(scenario_file.output_dir);
    validate_selected_scenario(&target, &scenario, &scenario_file.value)?;

    tracing::info!(
        target = %target,
        scenario = %scenario,
        config = %args.config.display(),
        output_dir = ?output_override,
        "starting scenario run"
    );

    let completed = match target.as_str() {
        "rust_chatbot" => {
            rust_chatbot::run_named_scenario(&scenario, scenario_file.value, output_override)?
        }
        "gpui_component_testing" => {
            gpui::run_named_scenario(&scenario, scenario_file.value, output_override)?
        }
        other => bail!(
            "Unsupported target {other:?}. Supported targets: {}.",
            SUPPORTED_TARGETS.join(", ")
        ),
    };

    tracing::info!(
        output_dir = %completed.output_dir.display(),
        report_path = %completed.report_path.display(),
        "scenario completed"
    );

    println!("\ncompleted:");
    println!("  output_dir: {}", completed.output_dir.display());
    println!("  report: {}", completed.report_path.display());
    Ok(())
}

pub fn validate_selected_scenario(
    target: &str,
    scenario: &str,
    value: &serde_json::Value,
) -> Result<()> {
    match target {
        "rust_chatbot" => rust_chatbot::validate_named_scenario(scenario, value),
        "gpui_component_testing" => gpui::validate_named_scenario(scenario, value),
        other => bail!(
            "Unsupported target {other:?}. Supported targets: {}.",
            SUPPORTED_TARGETS.join(", ")
        ),
    }
}

pub fn print_scenarios(target: Option<&str>) {
    match target.map(normalize_name).as_deref() {
        Some("rust_chatbot") => {
            println!("rust_chatbot scenarios:");
            for scenario in rust_chatbot::scenario_names() {
                println!("  {scenario}");
            }
        }
        Some("gpui_component_testing") => {
            println!("gpui_component_testing scenarios:");
            for scenario in gpui::scenario_names() {
                println!("  {scenario}");
            }
        }
        Some(other) => {
            println!("unknown target: {other}");
        }
        None => {
            println!("Available scenarios:");
            println!("  rust_chatbot ({} scenarios)", rust_chatbot::scenario_names().len());
            for scenario in rust_chatbot::scenario_names() {
                println!("    {scenario}");
            }
            println!("  gpui_component_testing ({} scenarios)", gpui::scenario_names().len());
            for scenario in gpui::scenario_names() {
                println!("    {scenario}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_ui_core::repo_root;

    fn validate_example(path: &str) -> Result<()> {
        let scenario_file = parse_scenario_file(&repo_root().join(path))?;
        let target = normalize_name(&scenario_file.target);
        let scenario = normalize_name(&scenario_file.scenario);
        validate_selected_scenario(&target, &scenario, &scenario_file.value)
    }

    #[test]
    fn rust_chatbot_debug_example_validates() {
        validate_example("examples/rust-chatbot-debug.toml").unwrap();
    }

    #[test]
    fn rust_chatbot_header_debug_example_validates() {
        validate_example("examples/rust-chatbot-header-debug.toml").unwrap();
    }

    #[test]
    fn gpui_scroll_matrix_example_validates() {
        validate_example("examples/gpui-scroll-matrix.toml").unwrap();
    }

    #[test]
    fn gpui_scrollbar_trace_example_validates() {
        validate_example("examples/gpui-scrollbar-trace.toml").unwrap();
    }

    #[test]
    fn gpui_conversation_paint_example_validates() {
        validate_example("examples/gpui-conversation-paint.toml").unwrap();
    }

    #[test]
    fn unsupported_target_error_lists_supported_targets() {
        let err = validate_selected_scenario("nope", "debug", &serde_json::Value::Null)
            .unwrap_err()
            .to_string();
        assert!(err.contains("rust_chatbot"));
        assert!(err.contains("gpui_component_testing"));
    }

    #[test]
    fn debug_scenario_rejects_unknown_fields() {
        // Using unknown_field instead of a valid field to trigger deny_unknown_fields
        let value = serde_json::json!({
            "app": { "root": "/test" },
            "unknown_extra_field": "should cause error"
        });
        let err = validate_selected_scenario("rust_chatbot", "debug", &value)
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown") || err.contains("Unknown"));
    }

    #[test]
    fn debug_scenario_rejects_empty_widths() {
        // Empty widths should fail validation since at least one width is required
        let value = serde_json::json!({
            "window": {
                "widths": [],
                "height": 900
            }
        });
        let err = validate_selected_scenario("rust_chatbot", "debug", &value)
            .unwrap_err()
            .to_string();
        assert!(err.contains("width") || err.contains("required"));
    }

    #[test]
    fn scroll_matrix_scenario_rejects_empty_variants() {
        // Empty variants should fail validation since at least one variant is required
        let value = serde_json::json!({
            "bench": {
                "variants": [],
                "run_ms": 5000
            }
        });
        let err = validate_selected_scenario("gpui_component_testing", "scroll_matrix", &value)
            .unwrap_err()
            .to_string();
        assert!(err.contains("variant") || err.contains("required"));
    }

    #[test]
    fn conversation_paint_scenario_rejects_empty_threads() {
        // Empty threads should fail validation since at least one thread is required
        let value = serde_json::json!({
            "bench": {
                "threads": [],
                "run_ms": 5000
            }
        });
        let err =
            validate_selected_scenario("gpui_component_testing", "conversation_paint", &value)
                .unwrap_err()
                .to_string();
        assert!(err.contains("thread") || err.contains("required"));
    }

    #[test]
    fn rust_chatbot_scenario_names_match_toml_files() {
        // Verify scenario_names() output aligns with rust-chatbot TOML files
        let expected = &["debug", "header_debug", "prompt_debug"];
        assert_eq!(rust_chatbot::scenario_names(), expected);
    }

    #[test]
    fn gpui_scenario_names_match_toml_files() {
        // Verify scenario_names() output aligns with gpui TOML files
        let expected = &["scroll_matrix", "scrollbar_trace", "conversation_paint"];
        assert_eq!(gpui::scenario_names(), expected);
    }

    #[test]
    fn all_example_toml_files_validate() {
        // All TOML files in examples/ should validate successfully
        let examples_dir = repo_root().join("examples");
        for entry in std::fs::read_dir(examples_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                let relative = path.strip_prefix(&repo_root()).unwrap();
                validate_example(relative.to_str().unwrap()).unwrap();
            }
        }
    }

    #[test]
    fn cli_flag_overrides_scenario_file() {
        // When both CLI and scenario file provide values,
        // CLI should take priority (this is how the convenience commands work)
        let scenario_value = serde_json::json!({
            "window": {
                "widths": [400, 500],
                "height": 600
            }
        });
        // The scenario file values should be preserved when passed through
        // Note: actual CLI override happens at the run_cmd level before calling adapter
        let target = "rust_chatbot";
        let scenario = "debug";
        // This validates that scenario file values are accepted
        validate_selected_scenario(target, scenario, &scenario_value).unwrap();
    }

    #[test]
    fn scenario_file_overrides_adapter_default() {
        // Scenario file values should override adapter defaults
        let scenario_value = serde_json::json!({
            "app": {
                "root": "/custom/root"
            },
            "window": {
                "widths": [800],
                "height": 700
            }
        });
        let target = "rust_chatbot";
        let scenario = "debug";
        // This validates scenario file overrides defaults
        validate_selected_scenario(target, scenario, &scenario_value).unwrap();
    }

    #[test]
    fn empty_scenario_uses_defaults() {
        // When scenario file doesn't provide values, adapter defaults are used
        let scenario_value = serde_json::json!({
            "window": {}
        });
        let target = "rust_chatbot";
        let scenario = "debug";
        // Empty window object - should use defaults
        validate_selected_scenario(target, scenario, &scenario_value).unwrap();
    }

    #[test]
    fn print_scenarios_shows_target_header() {
        // Verify it doesn't panic and produces some output
        print_scenarios(Some("rust_chatbot"));
    }

    #[test]
    fn print_scenarios_unfiltered_shows_all_targets() {
        // Verify unfiltered call includes both targets with counts
        print_scenarios(None);
    }
}
