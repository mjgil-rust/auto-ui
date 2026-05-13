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
            // Replace KEY=VALUE pattern with KEY=[REDACTED] to fully redact the secret
            // Find the pattern KEY= and replace up to the next whitespace or end of string
            let key_pattern = format!("{}=", key);
            let mut start = 0;
            while let Some(pos) = safe_error[start..].find(&key_pattern) {
                let abs_pos = start + pos;
                // Find end of value (next whitespace or end of string)
                let value_start = abs_pos + key_pattern.len();
                let value_end = safe_error[value_start..]
                    .find(|c: char| c.is_whitespace())
                    .map(|i| value_start + i)
                    .unwrap_or(safe_error.len());
                // Replace KEY=VALUE with KEY=[REDACTED]
                safe_error = format!(
                    "{}{}=[REDACTED]{}",
                    &safe_error[..abs_pos],
                    key,
                    &safe_error[value_end..]
                );
                // Move past this replacement to avoid infinite loop
                start = abs_pos + key.len() + "[REDACTED]".len();
            }
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

    #[test]
    fn format_safe_error_redacts_api_key_in_message() {
        // Set a sensitive env var for this test
        std::env::set_var("OPENAI_API_KEY", "sk-test-secret123");
        let result = format_safe_error("Failed: OPENAI_API_KEY=sk-test-secret123 is invalid");
        std::env::remove_var("OPENAI_API_KEY");

        assert!(
            result.contains("OPENAI_API_KEY=[REDACTED]"),
            "should redact the API key value, got: {}",
            result
        );
        assert!(
            !result.contains("sk-test-secret123"),
            "should not contain the secret value, got: {}",
            result
        );
    }

    #[test]
    fn format_safe_error_redacts_multiple_api_keys() {
        std::env::set_var("OPENAI_API_KEY", "sk-secret1");
        std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-secret2");
        let result = format_safe_error(
            "Auth failed: OPENAI_API_KEY=sk-secret1 and ANTHROPIC_API_KEY=sk-ant-secret2",
        );
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("ANTHROPIC_API_KEY");

        assert!(
            !result.contains("sk-secret1") && !result.contains("sk-ant-secret2"),
            "should redact both secrets, got: {}",
            result
        );
        assert!(
            result.contains("OPENAI_API_KEY=[REDACTED]")
                && result.contains("ANTHROPIC_API_KEY=[REDACTED]"),
            "should contain redaction markers, got: {}",
            result
        );
    }

    #[test]
    fn format_safe_error_handles_token_in_error() {
        std::env::set_var("SESSION_TOKEN", "tok_session_abc123xyz");
        let result = format_safe_error("Token error: SESSION_TOKEN=tok_session_abc123xyz expired");
        std::env::remove_var("SESSION_TOKEN");

        assert!(
            !result.contains("tok_session_abc123xyz"),
            "should not contain token value, got: {}",
            result
        );
        assert!(
            result.contains("SESSION_TOKEN=[REDACTED]"),
            "should contain redaction marker, got: {}",
            result
        );
    }

    #[test]
    fn format_safe_error_preserves_non_sensitive_content() {
        std::env::set_var("OPENAI_API_KEY", "sk-secret");
        let result = format_safe_error("Error code: 123, message: something went wrong");
        std::env::remove_var("OPENAI_API_KEY");

        // Should preserve the non-sensitive parts
        assert!(
            result.contains("Error code: 123"),
            "should preserve error code"
        );
        assert!(
            result.contains("message: something went wrong"),
            "should preserve error message"
        );
    }
}
