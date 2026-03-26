use std::path::PathBuf;

use anyhow::{bail, Result};
use auto_ui_adapter_gpui as gpui;
use auto_ui_adapter_rust_chatbot as rust_chatbot;
use auto_ui_core::{normalize_name, parse_scenario_file};
use clap::Args as ClapArgs;

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
}

pub fn run(args: Args) -> Result<()> {
    let scenario_file = parse_scenario_file(&args.config)?;
    let target = normalize_name(args.target.as_deref().unwrap_or(&scenario_file.target));
    let scenario = normalize_name(args.scenario.as_deref().unwrap_or(&scenario_file.scenario));
    let output_override = args.output_dir.or(scenario_file.output_dir);

    let completed = match target.as_str() {
        "rust_chatbot" => {
            rust_chatbot::run_named_scenario(&scenario, scenario_file.value, output_override)?
        }
        "gpui_component_testing" => {
            gpui::run_named_scenario(&scenario, scenario_file.value, output_override)?
        }
        other => bail!("Unsupported target {other:?}."),
    };

    println!("\ncompleted:");
    println!("  output_dir: {}", completed.output_dir.display());
    println!("  report: {}", completed.report_path.display());
    Ok(())
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
