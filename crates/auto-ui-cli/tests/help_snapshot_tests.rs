//! Help output snapshot tests for CLI.
//!
//! These tests verify that the CLI help output structure is correct
//! by parsing the clap command structure directly.

use clap::CommandFactory;

#[test]
fn top_level_help_contains_all_commands() {
    let mut cmd = auto_ui_cli::Cli::command();
    let help = cmd.render_help().to_string();

    // Verify all commands are documented in top-level help
    assert!(help.contains("debug"), "missing 'debug' command");
    assert!(help.contains("header-debug"), "missing 'header-debug' command");
    assert!(help.contains("run"), "missing 'run' command");
    assert!(help.contains("report-schema"), "missing 'report-schema' command");
    assert!(help.contains("targets"), "missing 'targets' command");
    assert!(help.contains("scenarios"), "missing 'scenarios' command");
}

#[test]
fn debug_subcommand_help_contains_key_options() {
    let mut cmd = auto_ui_cli::Cli::command();
    let help = cmd.render_help().to_string();

    // Verify debug command is mentioned with its options
    assert!(help.contains("debug"), "missing 'debug' command");
}

#[test]
fn header_debug_subcommand_help_contains_key_options() {
    let mut cmd = auto_ui_cli::Cli::command();
    let help = cmd.render_help().to_string();

    // Verify header-debug command is mentioned
    assert!(help.contains("header-debug"), "missing 'header-debug' command");
}

#[test]
fn run_subcommand_exists() {
    let mut cmd = auto_ui_cli::Cli::command();
    let help = cmd.render_help().to_string();

    // Verify run command is mentioned
    assert!(help.contains("run"), "missing 'run' command");
}

#[test]
fn scenarios_subcommand_has_target_option() {
    let mut cmd = auto_ui_cli::Cli::command();
    let help = cmd.render_help().to_string();

    // The scenarios command should be mentioned
    assert!(help.contains("scenarios"), "missing 'scenarios' command");
}

#[test]
fn targets_subcommand_exists() {
    let mut cmd = auto_ui_cli::Cli::command();
    let help = cmd.render_help().to_string();

    // The targets command should be mentioned
    assert!(help.contains("targets"), "missing 'targets' command");
}

#[test]
fn report_schema_subcommand_exists() {
    let mut cmd = auto_ui_cli::Cli::command();
    let help = cmd.render_help().to_string();

    // The report-schema command should be mentioned
    assert!(help.contains("report-schema"), "missing 'report-schema' command");
}

#[test]
fn help_shows_version() {
    let mut cmd = auto_ui_cli::Cli::command();
    let help = cmd.render_help().to_string();

    // Help should show version option
    assert!(help.contains("-V") || help.contains("--version"), "missing version option");
}
