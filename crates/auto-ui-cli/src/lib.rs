pub mod debug_cmd;
pub mod error_fmt;
pub mod header_debug_cmd;
pub mod inspect_report_cmd;
pub mod logging;
pub mod report_schema_cmd;
pub mod run_cmd;
pub mod scenario_schema_cmd;
pub mod validate_cmd;

use auto_ui_core::WindowDriver;
use clap::{Parser, Subcommand};

/// Build the platform-specific window driver.
/// Linux uses X11WindowDriver; macOS driver will be added in Phase 4.
#[cfg(target_os = "linux")]
pub fn build_driver() -> anyhow::Result<Box<dyn WindowDriver>> {
    let driver = auto_ui_driver_x11::X11WindowDriver::new();
    driver.check_required_tools()?;
    Ok(Box::new(driver))
}

#[cfg(target_os = "macos")]
pub fn build_driver() -> anyhow::Result<Box<dyn WindowDriver>> {
    let driver = auto_ui_driver_macos::MacOsWindowDriver::new();
    driver.check_required_tools()?;
    Ok(Box::new(driver))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn build_driver() -> anyhow::Result<Box<dyn WindowDriver>> {
    anyhow::bail!("unsupported platform: only Linux and macOS are supported")
}

#[derive(Parser)]
#[command(
    name = "auto-ui",
    version,
    about = "Generic desktop UI automation helpers"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run rust-chatbot debug session with window measurements
    Debug(debug_cmd::Args),
    /// Run rust-chatbot header debug with screenshot and crop metrics
    HeaderDebug(header_debug_cmd::Args),
    /// Run a generic scenario from a TOML config file
    Run(run_cmd::Args),
    /// Print the JSON schema for report files
    ReportSchema,
    /// List available automation targets (rust_chatbot, gpui_component_testing)
    Targets,
    /// List scenarios for a target (use --target to filter)
    Scenarios {
        #[arg(long)]
        target: Option<String>,
    },
    /// Inspect an existing report.json file
    Inspect(inspect_report_cmd::Args),
    /// Print the JSON schema for scenario configuration files
    ScenarioSchema(scenario_schema_cmd::ScenarioSchemaArgs),
    /// Validate a TOML scenario configuration file
    Validate(validate_cmd::ValidateScenarioArgs),
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Debug(args) => debug_cmd::run(args),
        Command::HeaderDebug(args) => header_debug_cmd::run(args),
        Command::Run(args) => run_cmd::run(args),
        Command::ReportSchema => report_schema_cmd::run(),
        Command::Targets => {
            println!("rust_chatbot");
            println!("gpui_component_testing");
            Ok(())
        }
        Command::Scenarios { target } => {
            run_cmd::print_scenarios(target.as_deref());
            Ok(())
        }
        Command::Inspect(args) => inspect_report_cmd::run(args),
        Command::ScenarioSchema(args) => scenario_schema_cmd::run(&args),
        Command::Validate(args) => validate_cmd::run(&args),
    }
}
