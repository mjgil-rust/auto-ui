use std::path::PathBuf;

use anyhow::{bail, Result};
use auto_ui_adapter_gpui as gpui;
use auto_ui_adapter_rust_chatbot as rust_chatbot;
use auto_ui_core::{normalize_name, parse_scenario_file, HeadlessDisplay};
use clap::Args as ClapArgs;

const SUPPORTED_TARGETS: &[&str] = &["rust_chatbot", "gpui_component_testing"];
const DEFAULT_GEOMETRY: &str = "1280x800x24";

#[derive(Clone, Debug, ClapArgs)]
pub struct Args {
    #[arg(long)]
    pub config: PathBuf,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub scenario: Option<String>,
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

    println!("\ncompleted:");
    println!("  output_dir: {}", completed.output_dir.display());
    println!("  report: {}", completed.report_path.display());
    Ok(())
}

fn validate_selected_scenario(
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
            for scenario in rust_chatbot::scenario_names() {
                println!("{scenario}");
            }
        }
        Some("gpui_component_testing") => {
            for scenario in gpui::scenario_names() {
                println!("{scenario}");
            }
        }
        Some(other) => {
            println!("unknown target: {other}");
        }
        None => {
            println!("rust_chatbot:");
            for scenario in rust_chatbot::scenario_names() {
                println!("  {scenario}");
            }
            println!("gpui_component_testing:");
            for scenario in gpui::scenario_names() {
                println!("  {scenario}");
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
        let err = validate_selected_scenario("gpui_component_testing", "conversation_paint", &value)
            .unwrap_err()
            .to_string();
        assert!(err.contains("thread") || err.contains("required"));
    }
}
