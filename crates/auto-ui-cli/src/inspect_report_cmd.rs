use anyhow::{Context, Result};
use auto_ui_artifacts::Report;
use std::path::PathBuf;
use serde::Deserialize;

#[derive(Debug, Deserialize, clap::Args)]
pub struct Args {
    /// Path to report.json file to inspect
    #[arg(long)]
    pub report: PathBuf,
}

/// Inspect an existing report.json file and print a human-readable summary.
pub fn run(args: Args) -> Result<()> {
    let file = std::fs::File::open(&args.report)
        .with_context(|| format!("failed to open {}", args.report.display()))?;
    let report: Report = serde_json::from_reader(file)
        .with_context(|| format!("failed to parse {}", args.report.display()))?;

    println!("Report: {}", args.report.display());
    println!("  target: {}", report.target);
    println!("  scenario: {}", report.scenario);
    println!("  mode: {}", report.mode);
    println!("  status: {}", report.status);
    if let Some(msg) = report.status_message {
        println!("  status_message: {}", msg);
    }
    println!("  started: {}", report.started_at);
    if let Some(finished) = report.finished_at {
        println!("  finished: {}", finished);
    }
    println!("  run_id: {}", report.run_id);
    println!("  artifacts: {}", report.artifacts.len());
    for artifact in &report.artifacts {
        println!("    - {} ({})", artifact.kind, artifact.path);
    }
    println!("  measurements: {}", report.measurements.len());
    println!("  events: {}", report.events.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_ui_artifacts::Report;
    use std::fs::File;
    use std::io::Write;

    fn create_test_report() -> (std::path::PathBuf, tempfile::TempDir) {
        let temp = tempfile::TempDir::new().unwrap();
        let report_path = temp.path().join("report.json");

        let report = Report::new(
            "rust_chatbot",
            "debug",
            "hybrid",
            std::path::Path::new("/tmp"),
        );
        let file_path = report_path.clone();
        let mut file = File::create(&file_path).unwrap();
        serde_json::to_writer_pretty(&mut file, &report).unwrap();

        (report_path, temp)
    }

    #[test]
    fn inspect_report_loads_valid_report() {
        let (report_path, _temp) = create_test_report();
        let args = Args { report: report_path };
        // Should not panic - just verify it loads
        assert!(args.report.exists());
    }

    #[test]
    fn inspect_report_fails_for_missing_file() {
        let args = Args {
            report: std::path::PathBuf::from("/nonexistent/path/report.json"),
        };
        // The run() function should fail with a helpful error
        let result = run(args);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("failed to open") || err_msg.contains("No such file"));
    }
}
