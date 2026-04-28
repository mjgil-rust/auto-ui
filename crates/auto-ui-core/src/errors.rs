//! Explicit error types for auto-ui operations.
//!
//! This module provides structured error types using thiserror,
//! allowing callers to match on specific error variants rather
//! than relying on string matching or broad anyhow::Error handling.

use std::path::PathBuf;
use thiserror::Error;

/// Error when a required target app root cannot be found.
#[derive(Debug, Error)]
#[error("target root not found: {path} ({reason})")]
pub struct TargetRootError {
    pub path: PathBuf,
    pub reason: String,
}

/// Error when a required binary is missing or not executable.
#[derive(Debug, Error)]
#[error("binary not found or not executable: {path}")]
pub struct BinaryNotFoundError {
    pub path: PathBuf,
}

/// Error when a window cannot be found.
#[derive(Debug, Error)]
#[error("window not found: {window_id}")]
pub struct WindowNotFoundError {
    pub window_id: String,
}

/// Error when trace parsing/reading times out.
#[derive(Debug, Error)]
#[error("trace timeout after {timeout_secs}s: {reason}")]
pub struct TraceTimeoutError {
    pub timeout_secs: u64,
    pub reason: String,
}

/// Error when an expected artifact is missing.
#[derive(Debug, Error)]
#[error("artifact missing: {kind} at {path}")]
pub struct ArtifactMissingError {
    pub kind: String,
    pub path: PathBuf,
}

/// Error when a managed process exits unexpectedly.
#[derive(Debug, Error)]
#[error("process exited unexpectedly with code {exit_code} (expected {expected})")]
pub struct ProcessExitedError {
    pub exit_code: i32,
    pub expected: &'static str,
}

/// Error when report serialization fails.
#[derive(Debug, Error)]
#[error("report serialization failed: {0}")]
pub struct ReportSerializationError(pub String);

/// Error when display initialization fails.
#[derive(Debug, Error)]
#[error("display error: {0}")]
pub struct DisplayError(pub String);

/// Error when path expansion fails.
#[derive(Debug, Error)]
#[error("path expansion failed for '{input}': {reason}")]
pub struct PathExpansionError {
    pub input: String,
    pub reason: String,
}

/// Error when window operations fail.
#[derive(Debug, Error)]
#[error("window operation '{operation}' failed for {window_id}: {reason}")]
pub struct WindowOperationError {
    pub operation: String,
    pub window_id: String,
    pub reason: String,
}

/// Error when scenario validation fails.
#[derive(Debug, Error)]
#[error("scenario validation failed: {0}")]
pub struct ScenarioValidationError(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_root_error_display() {
        let err = TargetRootError {
            path: PathBuf::from("/nonexistent"),
            reason: "directory does not exist".to_string(),
        };
        assert!(err.to_string().contains("/nonexistent"));
        assert!(err.to_string().contains("directory does not exist"));
    }

    #[test]
    fn binary_not_found_error_display() {
        let err = BinaryNotFoundError {
            path: PathBuf::from("/usr/bin/missing"),
        };
        assert!(err.to_string().contains("/usr/bin/missing"));
    }

    #[test]
    fn window_not_found_error_display() {
        let err = WindowNotFoundError {
            window_id: "0x123456".to_string(),
        };
        assert!(err.to_string().contains("0x123456"));
    }

    #[test]
    fn trace_timeout_error_display() {
        let err = TraceTimeoutError {
            timeout_secs: 30,
            reason: "no trace data received".to_string(),
        };
        assert!(err.to_string().contains("30s"));
        assert!(err.to_string().contains("no trace data received"));
    }

    #[test]
    fn artifact_missing_error_display() {
        let err = ArtifactMissingError {
            kind: "progress_log".to_string(),
            path: PathBuf::from("/tmp/missing.log"),
        };
        assert!(err.to_string().contains("progress_log"));
        assert!(err.to_string().contains("/tmp/missing.log"));
    }

    #[test]
    fn process_exited_error_display() {
        let err = ProcessExitedError {
            exit_code: 1,
            expected: "exit code 0",
        };
        assert!(err.to_string().contains("1"));
        assert!(err.to_string().contains("exit code 0"));
    }

    #[test]
    fn report_serialization_error_display() {
        let err = ReportSerializationError("JSON serialization failed".to_string());
        assert!(err.to_string().contains("JSON serialization failed"));
    }

    #[test]
    fn display_error_display() {
        let err = DisplayError("cannot open display".to_string());
        assert!(err.to_string().contains("cannot open display"));
    }

    #[test]
    fn path_expansion_error_display() {
        let err = PathExpansionError {
            input: "~".to_string(),
            reason: "HOME not set".to_string(),
        };
        assert!(err.to_string().contains("~"));
        assert!(err.to_string().contains("HOME not set"));
    }

    #[test]
    fn window_operation_error_display() {
        let err = WindowOperationError {
            operation: "resize".to_string(),
            window_id: "0x123".to_string(),
            reason: "invalid geometry".to_string(),
        };
        assert!(err.to_string().contains("resize"));
        assert!(err.to_string().contains("0x123"));
        assert!(err.to_string().contains("invalid geometry"));
    }

    #[test]
    fn scenario_validation_error_display() {
        let err = ScenarioValidationError("missing required field 'widths'".to_string());
        assert!(err.to_string().contains("missing required field"));
    }
}
