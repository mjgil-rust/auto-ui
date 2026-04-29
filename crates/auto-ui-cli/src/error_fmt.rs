//! Error formatting for CLI failures.
//!
//! Provides structured error messages that point users to relevant
//! diagnostic artifacts when commands fail.

use std::path::Path;

use auto_ui_artifacts::Report;

use auto_ui_core::safe_log::redact_env_vars;

/// Format a failure summary that points to diagnostic artifacts.
/// Includes progress log path, report path (if available), and troubleshooting hints.
pub fn format_failure_summary(
    error: &str,
    output_dir: Option<&Path>,
    progress_log_path: Option<&Path>,
    report: Option<&Report>,
) -> String {
    let mut summary = String::new();
    summary.push_str(&format!("error: {}\n\n", error));

    // Add report path if available
    if let Some(report) = report {
        summary.push_str(&format!(
            "report: {}\n",
            report.report_path(Path::new(&report.app_root)).display()
        ));
    }

    // Add output directory if available
    if let Some(dir) = output_dir {
        summary.push_str(&format!("output_dir: {}\n", dir.display()));
    }

    // Add progress log path if available
    if let Some(log_path) = progress_log_path {
        summary.push_str(&format!("progress_log: {}\n", log_path.display()));
    }

    // Add troubleshooting hint
    summary.push_str("\nFor more details, check the progress log and report.");

    summary
}

/// Format an error with safe env var redaction for display.
/// Removes sensitive values from env vars before showing to user.
pub fn format_safe_error(error: &str) -> String {
    // Redact env vars in error message for safety
    let redacted = redact_env_vars();
    let mut safe_error = error.to_string();

    for (key, value) in redacted.iter() {
        if value == "[REDACTED]" {
            // Redacted env var - replace any occurrence of the key with placeholder
            safe_error = safe_error.replace(key, &format!("{}=[REDACTED]", key));
        }
    }

    safe_error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_failure_summary_includes_error() {
        let summary = format_failure_summary(
            "trace timeout after 30s",
            Some(Path::new("/tmp/output")),
            Some(Path::new("/tmp/output/progress.log")),
            None,
        );

        assert!(summary.contains("trace timeout after 30s"));
        assert!(summary.contains("/tmp/output"));
        assert!(summary.contains("progress.log"));
        assert!(summary.contains("For more details"));
    }

    #[test]
    fn format_safe_error_does_not_panic() {
        // Should not panic even with unusual characters
        let result = format_safe_error("test error with special chars: <>&\"'");
        assert!(result.contains("test error"));
    }
}