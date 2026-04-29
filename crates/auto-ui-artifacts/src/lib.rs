use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const REPORT_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub kind: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: String,
    pub tool_version: String,
    pub target: String,
    pub scenario: String,
    pub mode: String,
    pub app_root: String,
    pub run_id: String,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactRef>,
    #[serde(default)]
    pub measurements: Vec<Value>,
    #[serde(default)]
    pub events: Vec<Value>,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub details: Value,
}

impl Report {
    pub fn new(target: &str, scenario: &str, mode: &str, app_root: &Path) -> Self {
        let started_at = Utc::now();
        Self {
            schema_version: REPORT_SCHEMA_VERSION.to_string(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            target: target.to_string(),
            scenario: scenario.to_string(),
            mode: mode.to_string(),
            app_root: app_root.display().to_string(),
            run_id: started_at.format("%Y%m%d-%H%M%S").to_string(),
            started_at: started_at.to_rfc3339(),
            finished_at: None,
            status: "running".to_string(),
            status_message: None,
            artifacts: Vec::new(),
            measurements: Vec::new(),
            events: Vec::new(),
            details: Value::Object(Default::default()),
        }
    }

    pub fn add_artifact(
        &mut self,
        kind: impl Into<String>,
        path: impl AsRef<Path>,
        description: Option<String>,
        metadata: Value,
    ) {
        self.artifacts.push(ArtifactRef {
            kind: kind.into(),
            path: normalize_artifact_path(path.as_ref()),
            description,
            metadata,
        });
    }

    pub fn push_measurement(&mut self, value: Value) {
        self.measurements.push(value);
    }

    pub fn push_event(&mut self, value: Value) {
        self.events.push(value);
    }

    /// Adds a lifecycle event (prepare, launch, collect, stop, etc.).
    pub fn push_lifecycle_event(&mut self, phase: &str, message: &str) {
        self.events.push(json!({
            "timestamp": Utc::now().to_rfc3339(),
            "kind": "lifecycle",
            "phase": phase,
            "message": message,
        }));
    }

    /// Adds a retry event when an operation is retried.
    pub fn push_retry_event(&mut self, operation: &str, attempt: u32, max_attempts: u32) {
        self.events.push(json!({
            "timestamp": Utc::now().to_rfc3339(),
            "kind": "retry",
            "operation": operation,
            "attempt": attempt,
            "max_attempts": max_attempts,
        }));
    }

    /// Adds a timeout event when an operation times out.
    pub fn push_timeout_event(&mut self, operation: &str, timeout_secs: u64) {
        self.events.push(json!({
            "timestamp": Utc::now().to_rfc3339(),
            "kind": "timeout",
            "operation": operation,
            "timeout_secs": timeout_secs,
        }));
    }

    /// Adds a foreground-control event (activate, lower, etc.).
    pub fn push_foreground_event(&mut self, action: &str, window_id: &str) {
        self.events.push(json!({
            "timestamp": Utc::now().to_rfc3339(),
            "kind": "foreground_control",
            "action": action,
            "window_id": window_id,
        }));
    }

    /// Adds an import event when artifacts are imported.
    pub fn push_import_event(&mut self, artifact_kind: &str, path: &str) {
        self.events.push(json!({
            "timestamp": Utc::now().to_rfc3339(),
            "kind": "import",
            "artifact_kind": artifact_kind,
            "path": path,
        }));
    }

    /// Adds a window event (resize, screenshot, etc.).
    pub fn push_window_event(&mut self, action: &str, window_id: &str, details: Value) {
        self.events.push(json!({
            "timestamp": Utc::now().to_rfc3339(),
            "kind": "window",
            "action": action,
            "window_id": window_id,
            "details": details,
        }));
    }

    pub fn set_details(&mut self, value: Value) {
        self.details = value;
    }

    pub fn finish_ok(&mut self) {
        self.finished_at = Some(Utc::now().to_rfc3339());
        self.status = "ok".to_string();
        self.status_message = None;
    }

    pub fn finish_error(&mut self, message: impl Into<String>) {
        self.finished_at = Some(Utc::now().to_rfc3339());
        self.status = "error".to_string();
        self.status_message = Some(message.into());
    }

    pub fn report_path(&self, output_dir: &Path) -> PathBuf {
        output_dir.join("report.json")
    }
}

pub fn report_schema_json() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://auto-ui.local/report.schema.json",
        "title": "auto-ui report",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schema_version",
            "tool_version",
            "target",
            "scenario",
            "mode",
            "app_root",
            "run_id",
            "started_at",
            "status",
            "artifacts",
            "measurements",
            "events",
            "details"
        ],
        "properties": {
            "schema_version": {
                "type": "string",
                "const": REPORT_SCHEMA_VERSION,
                "description": "Stable top-level report schema version."
            },
            "tool_version": {
                "type": "string",
                "description": "Version of the auto-ui binary that wrote the report."
            },
            "target": {
                "type": "string",
                "description": "Normalized target id, for example rust_chatbot."
            },
            "scenario": {
                "type": "string",
                "description": "Normalized scenario id, for example scroll_matrix."
            },
            "mode": {
                "type": "string",
                "description": "Execution mode chosen by the adapter."
            },
            "app_root": {
                "type": "string",
                "description": "Resolved target checkout or app root used for the run."
            },
            "run_id": {
                "type": "string",
                "description": "Run identifier scoped to the output directory."
            },
            "started_at": {
                "type": "string",
                "format": "date-time"
            },
            "finished_at": {
                "anyOf": [
                    { "type": "string", "format": "date-time" },
                    { "type": "null" }
                ]
            },
            "status": {
                "type": "string",
                "enum": ["running", "ok", "error"]
            },
            "status_message": {
                "anyOf": [
                    { "type": "string" },
                    { "type": "null" }
                ]
            },
            "artifacts": {
                "type": "array",
                "items": { "$ref": "#/$defs/artifact_ref" }
            },
            "measurements": {
                "type": "array",
                "items": {
                    "type": "object",
                    "description": "Shared measurement rows. Adapters may add extra keys."
                }
            },
            "events": {
                "type": "array",
                "items": {
                    "type": "object",
                    "description": "Timeline or lifecycle events. Adapters may add extra keys."
                }
            },
            "details": {
                "description": "Adapter-specific structured JSON payload."
            }
        },
        "$defs": {
            "artifact_ref": {
                "type": "object",
                "additionalProperties": false,
                "required": ["kind", "path"],
                "properties": {
                    "kind": { "type": "string" },
                    "path": { "type": "string" },
                    "description": { "type": "string" },
                    "metadata": {
                        "anyOf": [
                            { "type": "object" },
                            { "type": "null" }
                        ]
                    }
                }
            }
        }
    })
}

pub fn write_report(output_dir: &Path, report: &Report) -> Result<PathBuf> {
    let report_path = report.report_path(output_dir);
    fs::write(&report_path, serde_json::to_string_pretty(report)?)
        .with_context(|| format!("failed to write {}", report_path.display()))?;
    Ok(report_path)
}

/// Normalize an artifact path: convert to absolute path.
/// Relative paths are resolved from the current working directory.
/// Symlinks are resolved to their canonical form.
pub fn normalize_artifact_path<P: AsRef<Path>>(path: P) -> String {
    let p = path.as_ref();
    // If already absolute, canonicalize it
    if p.is_absolute() {
        std::fs::canonicalize(p)
            .map(|c| c.to_string_lossy().to_string())
            .unwrap_or_else(|_| p.to_string_lossy().to_string())
    } else {
        // Resolve relative path from current directory, then canonicalize
        std::env::current_dir()
            .map(|cwd| cwd.join(p))
            .and_then(|c| std::fs::canonicalize(&c))
            .map(|c| c.to_string_lossy().to_string())
            .unwrap_or_else(|_| {
                std::env::current_dir()
                    .map(|cwd| cwd.join(p).to_string_lossy().to_string())
                    .unwrap_or_else(|_| p.to_string_lossy().to_string())
            })
    }
}

/// Validate that all artifact paths exist on disk.
/// Returns Ok(()) if all artifacts exist, or Err with details about missing ones.
pub fn validate_artifacts(report: &Report) -> Result<()> {
    let mut missing = Vec::new();
    for artifact in &report.artifacts {
        if !Path::new(&artifact.path).exists() {
            missing.push(artifact.path.clone());
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(
            "missing {} artifact(s): {}",
            missing.len(),
            missing.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("auto-ui-{name}-{nanos}-{}", std::process::id()))
    }

    fn fixed_report() -> Report {
        Report {
            schema_version: REPORT_SCHEMA_VERSION.to_string(),
            tool_version: "0.1.0".to_string(),
            target: "rust_chatbot".to_string(),
            scenario: "debug".to_string(),
            mode: "hybrid".to_string(),
            app_root: "/tmp/app".to_string(),
            run_id: "20260326-120000".to_string(),
            started_at: "2026-03-26T12:00:00Z".to_string(),
            finished_at: Some("2026-03-26T12:00:01Z".to_string()),
            status: "ok".to_string(),
            status_message: None,
            artifacts: vec![ArtifactRef {
                kind: "progress_log".to_string(),
                path: "/tmp/out/progress.log".to_string(),
                description: Some("live progress log".to_string()),
                metadata: Value::Null,
            }],
            measurements: vec![json!({"name": "sample", "value": 1})],
            events: vec![json!({"kind": "started"})],
            details: json!({"widths": [520, 900]}),
        }
    }

    #[test]
    fn report_json_matches_expected_schema_shape() {
        let actual = serde_json::to_value(fixed_report()).unwrap();
        let expected = json!({
            "schema_version": "1",
            "tool_version": "0.1.0",
            "target": "rust_chatbot",
            "scenario": "debug",
            "mode": "hybrid",
            "app_root": "/tmp/app",
            "run_id": "20260326-120000",
            "started_at": "2026-03-26T12:00:00Z",
            "finished_at": "2026-03-26T12:00:01Z",
            "status": "ok",
            "artifacts": [
                {
                    "kind": "progress_log",
                    "path": "/tmp/out/progress.log",
                    "description": "live progress log"
                }
            ],
            "measurements": [
                {
                    "name": "sample",
                    "value": 1
                }
            ],
            "events": [
                {
                    "kind": "started"
                }
            ],
            "details": {
                "widths": [520, 900]
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn finish_methods_update_status_fields() {
        let mut report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/tmp/app"));
        assert_eq!(report.schema_version, REPORT_SCHEMA_VERSION);
        report.finish_error("failed");
        assert_eq!(report.status, "error");
        assert_eq!(report.status_message.as_deref(), Some("failed"));
        assert!(report.finished_at.is_some());

        report.finish_ok();
        assert_eq!(report.status, "ok");
        assert_eq!(report.status_message, None);
        assert!(report.finished_at.is_some());
    }

    #[test]
    fn write_report_writes_expected_json() {
        let output_dir = unique_temp_dir("report");
        fs::create_dir_all(&output_dir).unwrap();
        let report = fixed_report();

        let path = write_report(&output_dir, &report).unwrap();
        assert_eq!(path, output_dir.join("report.json"));

        let written: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let expected = serde_json::to_value(report).unwrap();
        assert_eq!(written, expected);

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(output_dir);
    }

    #[test]
    fn write_report_on_error_status_includes_error_details() {
        let output_dir = unique_temp_dir("report-error");
        fs::create_dir_all(&output_dir).unwrap();

        let mut report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/tmp/app"));
        report.finish_error("trace timeout after 30s");
        report.push_measurement(json!({"error": "timeout"}));

        let path = write_report(&output_dir, &report).unwrap();

        let written: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written.get("status").and_then(Value::as_str), Some("error"));
        assert_eq!(
            written.get("status_message").and_then(Value::as_str),
            Some("trace timeout after 30s")
        );
        assert!(written.get("finished_at").is_some());
        assert!(written
            .get("measurements")
            .and_then(Value::as_array)
            .is_some());

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(output_dir);
    }

    #[test]
    fn report_schema_declares_required_top_level_fields() {
        let schema = report_schema_json();
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();

        assert!(required.contains(&"schema_version"));
        assert!(required.contains(&"artifacts"));
        assert!(required.contains(&"measurements"));
        assert!(required.contains(&"events"));
        assert!(required.contains(&"details"));
        assert_eq!(
            schema
                .get("properties")
                .and_then(|value| value.get("schema_version"))
                .and_then(|value| value.get("const"))
                .and_then(Value::as_str),
            Some(REPORT_SCHEMA_VERSION)
        );
    }

    #[test]
    fn complete_report_conforms_to_schema() {
        // Build a complete, well-formed report and verify it passes schema checks
        let mut report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/test/app"));
        report.add_artifact(
            "progress_log",
            "/tmp/out/progress.log",
            Some("live progress log".to_string()),
            Value::Null,
        );
        report.add_artifact(
            "trace_log",
            "/tmp/out/trace.log",
            Some("rust-chatbot trace log".to_string()),
            Value::Null,
        );
        report.push_lifecycle_event("prepare", "adapter prepared");
        report.push_lifecycle_event("launch", "process launched");
        report.push_measurement(json!({"name": "session_count", "value": 3}));
        report.set_details(json!({
            "provider": "codex",
            "widths": [520, 900],
            "height": 900,
        }));
        report.finish_ok();

        let json = serde_json::to_value(&report).unwrap();

        // Validate required fields
        let required_fields = [
            "schema_version", "tool_version", "target", "scenario",
            "mode", "app_root", "run_id", "started_at", "status",
            "artifacts", "measurements", "events", "details",
        ];
        for field in required_fields {
            assert!(json.get(field).is_some(), "required field '{}' must be present", field);
        }

        // Validate status is one of the allowed values
        let status = json.get("status").and_then(Value::as_str).unwrap();
        assert!(
            status == "running" || status == "ok" || status == "error",
            "status must be one of 'running', 'ok', 'error', got '{}'",
            status
        );

        // Validate artifacts is an array with properly structured items
        let artifacts = json.get("artifacts").unwrap().as_array().unwrap();
        assert!(!artifacts.is_empty(), "artifacts should not be empty for this test");
        for art in artifacts {
            assert!(art.get("kind").is_some(), "artifact must have 'kind'");
            assert!(art.get("path").is_some(), "artifact must have 'path'");
        }

        // Validate events is an array
        let events = json.get("events").unwrap().as_array().unwrap();
        assert!(!events.is_empty(), "events should not be empty for this test");
        for evt in events {
            assert!(evt.get("kind").is_some(), "event must have 'kind'");
            assert!(evt.get("timestamp").is_some(), "event must have 'timestamp'");
        }

        // Validate measurements is an array
        let measurements = json.get("measurements").unwrap().as_array().unwrap();
        for m in measurements {
            assert!(m.get("name").is_some() || m.get("value").is_some(),
                "measurement should have 'name' and/or 'value'");
        }

        // Validate details is present and non-null
        assert!(
            !json.get("details").unwrap().is_null(),
            "details must not be null"
        );

        // Validate finished_at is present after finish_ok
        assert!(json.get("finished_at").is_some(), "finished_at must be present after finish");
    }

    #[test]
    fn error_report_conforms_to_schema() {
        let mut report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/test/app"));
        report.finish_error("trace timeout after 30s");

        let json = serde_json::to_value(&report).unwrap();

        // Status should be 'error'
        assert_eq!(json.get("status").and_then(Value::as_str), Some("error"));
        // status_message should be present for error reports
        assert!(json.get("status_message").is_some(), "error reports should have status_message");
        assert!(!json.get("status_message").unwrap().is_null(), "status_message should not be null for error");
        // finished_at should be present
        assert!(json.get("finished_at").is_some(), "finished_at must be present for completed report");
    }

    #[test]
    fn running_report_conforms_to_schema() {
        let report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/test/app"));

        let json = serde_json::to_value(&report).unwrap();

        // Status should be 'running' for unfinished reports
        assert_eq!(json.get("status").and_then(Value::as_str), Some("running"));
        // finished_at is skipped when None, so it should not be present
        assert!(json.get("finished_at").is_none(), "running reports should skip finished_at");
    }

    #[test]
    fn push_lifecycle_event_adds_event_with_phase_and_message() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        report.push_lifecycle_event("prepare", "adapter prepared");
        assert_eq!(report.events.len(), 1);
        let event = &report.events[0];
        assert_eq!(event.get("kind").and_then(Value::as_str), Some("lifecycle"));
        assert_eq!(event.get("phase").and_then(Value::as_str), Some("prepare"));
        assert_eq!(
            event.get("message").and_then(Value::as_str),
            Some("adapter prepared")
        );
        assert!(event.get("timestamp").is_some());
    }

    #[test]
    fn push_retry_event_adds_event_with_operation_and_attempts() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        report.push_retry_event("window_discovery", 3, 10);
        assert_eq!(report.events.len(), 1);
        let event = &report.events[0];
        assert_eq!(event.get("kind").and_then(Value::as_str), Some("retry"));
        assert_eq!(
            event.get("operation").and_then(Value::as_str),
            Some("window_discovery")
        );
        assert_eq!(
            event
                .get("attempt")
                .and_then(Value::as_u64)
                .map(|n| n as u32),
            Some(3)
        );
        assert_eq!(
            event
                .get("max_attempts")
                .and_then(Value::as_u64)
                .map(|n| n as u32),
            Some(10)
        );
    }

    #[test]
    fn push_timeout_event_adds_event_with_operation_and_timeout() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        report.push_timeout_event("trace_wait", 30);
        assert_eq!(report.events.len(), 1);
        let event = &report.events[0];
        assert_eq!(event.get("kind").and_then(Value::as_str), Some("timeout"));
        assert_eq!(
            event.get("operation").and_then(Value::as_str),
            Some("trace_wait")
        );
        assert_eq!(event.get("timeout_secs").and_then(Value::as_u64), Some(30));
    }

    #[test]
    fn push_foreground_event_adds_event_with_action_and_window() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        report.push_foreground_event("lower", "0x123456");
        assert_eq!(report.events.len(), 1);
        let event = &report.events[0];
        assert_eq!(
            event.get("kind").and_then(Value::as_str),
            Some("foreground_control")
        );
        assert_eq!(event.get("action").and_then(Value::as_str), Some("lower"));
        assert_eq!(
            event.get("window_id").and_then(Value::as_str),
            Some("0x123456")
        );
    }

    #[test]
    fn push_import_event_adds_event_with_artifact_kind_and_path() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        report.push_import_event("progress_log", "/tmp/run/progress.log");
        assert_eq!(report.events.len(), 1);
        let event = &report.events[0];
        assert_eq!(event.get("kind").and_then(Value::as_str), Some("import"));
        assert_eq!(
            event.get("artifact_kind").and_then(Value::as_str),
            Some("progress_log")
        );
        assert_eq!(
            event.get("path").and_then(Value::as_str),
            Some("/tmp/run/progress.log")
        );
    }

    #[test]
    fn push_window_event_adds_event_with_action_and_details() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        let details = json!({"width": 1280, "height": 800});
        report.push_window_event("resize", "0x789abc", details.clone());
        assert_eq!(report.events.len(), 1);
        let event = &report.events[0];
        assert_eq!(event.get("kind").and_then(Value::as_str), Some("window"));
        assert_eq!(event.get("action").and_then(Value::as_str), Some("resize"));
        assert_eq!(
            event.get("window_id").and_then(Value::as_str),
            Some("0x789abc")
        );
        let event_details = event.get("details");
        assert_eq!(
            serde_json::to_string(&event_details).unwrap(),
            serde_json::to_string(&Some(details.clone())).unwrap()
        );
    }

    // Event schema conventions tests

    #[test]
    fn lifecycle_events_emit_all_required_fields() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        report.push_lifecycle_event("prepare", "adapter initialized");
        report.push_lifecycle_event("launch", "process spawned");
        report.push_lifecycle_event("collect", "artifacts gathered");
        report.push_lifecycle_event("stop", "cleanup complete");

        assert_eq!(report.events.len(), 4);

        // Verify all lifecycle events have required fields
        for (i, phase) in ["prepare", "launch", "collect", "stop"].iter().enumerate() {
            let event = &report.events[i];
            assert_eq!(
                event.get("kind").and_then(Value::as_str),
                Some("lifecycle"),
                "event {} should have kind 'lifecycle'",
                i
            );
            assert_eq!(
                event.get("phase").and_then(Value::as_str),
                Some(*phase),
                "event {} should have phase '{}'",
                i,
                phase
            );
            assert!(
                event.get("timestamp").is_some(),
                "event {} should have timestamp",
                i
            );
        }
    }

    #[test]
    fn retry_events_emit_all_required_fields() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        report.push_retry_event("window_discovery", 2, 5);

        let event = &report.events[0];
        assert_eq!(event.get("kind").and_then(Value::as_str), Some("retry"));
        assert_eq!(
            event.get("operation").and_then(Value::as_str),
            Some("window_discovery")
        );
        assert_eq!(
            event.get("attempt").and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            event.get("max_attempts").and_then(Value::as_u64),
            Some(5)
        );
        assert!(event.get("timestamp").is_some());
    }

    #[test]
    fn timeout_events_emit_all_required_fields() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        report.push_timeout_event("trace_wait", 30);

        let event = &report.events[0];
        assert_eq!(event.get("kind").and_then(Value::as_str), Some("timeout"));
        assert_eq!(
            event.get("operation").and_then(Value::as_str),
            Some("trace_wait")
        );
        assert_eq!(
            event.get("timeout_secs").and_then(Value::as_u64),
            Some(30)
        );
        assert!(event.get("timestamp").is_some());
    }

    #[test]
    fn foreground_control_events_emit_all_required_fields() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "interactive_window",
            Path::new("/tmp"),
        );
        report.push_foreground_event("lower", "0x123abc");
        report.push_foreground_event("activate", "0x456def");

        assert_eq!(report.events.len(), 2);

        let lower_event = &report.events[0];
        assert_eq!(
            lower_event.get("kind").and_then(Value::as_str),
            Some("foreground_control")
        );
        assert_eq!(
            lower_event.get("action").and_then(Value::as_str),
            Some("lower")
        );
        assert_eq!(
            lower_event.get("window_id").and_then(Value::as_str),
            Some("0x123abc")
        );
        assert!(lower_event.get("timestamp").is_some());

        let activate_event = &report.events[1];
        assert_eq!(
            activate_event.get("action").and_then(Value::as_str),
            Some("activate")
        );
        assert_eq!(
            activate_event.get("window_id").and_then(Value::as_str),
            Some("0x456def")
        );
    }

    #[test]
    fn import_events_emit_all_required_fields() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        report.push_import_event("progress_log", "/tmp/out/progress.log");
        report.push_import_event("summary_csv", "/tmp/out/summary.csv");

        assert_eq!(report.events.len(), 2);

        for (i, expected_kind) in ["progress_log", "summary_csv"].iter().enumerate() {
            let event = &report.events[i];
            assert_eq!(
                event.get("kind").and_then(Value::as_str),
                Some("import"),
                "event {} should have kind 'import'",
                i
            );
            assert_eq!(
                event.get("artifact_kind").and_then(Value::as_str),
                Some(*expected_kind),
                "event {} should have artifact_kind '{}'",
                i,
                expected_kind
            );
            assert!(
                event.get("path").and_then(Value::as_str).is_some(),
                "event {} should have path",
                i
            );
            assert!(event.get("timestamp").is_some());
        }
    }

    #[test]
    fn window_events_emit_all_required_fields() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "interactive_window",
            Path::new("/tmp"),
        );
        let details = json!({"width": 1280, "height": 800});
        report.push_window_event("resize", "0x789abc", details.clone());

        let event = &report.events[0];
        assert_eq!(event.get("kind").and_then(Value::as_str), Some("window"));
        assert_eq!(
            event.get("action").and_then(Value::as_str),
            Some("resize")
        );
        assert_eq!(
            event.get("window_id").and_then(Value::as_str),
            Some("0x789abc")
        );
        assert_eq!(
            event.get("details").and_then(Value::as_object),
            details.as_object()
        );
        assert!(event.get("timestamp").is_some());
    }

    #[test]
    fn events_are_ordered_by_timestamp() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        // Push events in a known order
        report.push_lifecycle_event("prepare", "started");
        std::thread::sleep(std::time::Duration::from_millis(10));
        report.push_lifecycle_event("launch", "started");
        std::thread::sleep(std::time::Duration::from_millis(10));
        report.push_retry_event("discovery", 1, 3);
        std::thread::sleep(std::time::Duration::from_millis(10));
        report.push_timeout_event("wait", 5);

        assert_eq!(report.events.len(), 4);

        // Extract timestamps and verify they are increasing
        let timestamps: Vec<String> = report
            .events
            .iter()
            .filter_map(|e| e.get("timestamp").and_then(Value::as_str).map(String::from))
            .collect();

        assert!(
            timestamps.windows(2).all(|w| w[0] <= w[1]),
            "timestamps should be in ascending order: {:?}",
            timestamps
        );
    }

    #[test]
    fn multiple_events_are_accumulated_in_order() {
        let mut report = Report::new(
            "test_target",
            "test_scenario",
            "startup_driven",
            Path::new("/tmp"),
        );
        report.push_lifecycle_event("prepare", "started");
        report.push_lifecycle_event("launch", "started");
        report.push_retry_event("discovery", 1, 5);
        assert_eq!(report.events.len(), 3);
        assert_eq!(
            report.events[0].get("phase").and_then(Value::as_str),
            Some("prepare")
        );
        assert_eq!(
            report.events[1].get("phase").and_then(Value::as_str),
            Some("launch")
        );
        assert_eq!(
            report.events[2].get("operation").and_then(Value::as_str),
            Some("discovery")
        );
    }

    // Golden tests for scenario-specific report structures

    #[test]
    fn golden_rust_chatbot_debug_report_structure() {
        let mut report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/test/app"));
        report.add_artifact(
            "progress_log",
            "/tmp/out/progress.log",
            Some("live progress log".to_string()),
            Value::Null,
        );
        report.add_artifact(
            "trace_log",
            "/tmp/out/trace.log",
            Some("rust-chatbot trace log".to_string()),
            Value::Null,
        );
        report.push_lifecycle_event("prepare", "adapter prepared");
        report.push_lifecycle_event("launch", "process launched");
        report.set_details(json!({
            "provider": "codex",
            "instance": 1,
            "widths": [520, 900, 1000],
            "height": 900,
        }));
        report.finish_ok();

        let json = serde_json::to_value(&report).unwrap();

        // Verify all required top-level fields are present
        assert!(json.get("schema_version").is_some(), "schema_version required");
        assert!(json.get("tool_version").is_some(), "tool_version required");
        assert_eq!(
            json.get("target").and_then(Value::as_str),
            Some("rust_chatbot")
        );
        assert_eq!(json.get("scenario").and_then(Value::as_str), Some("debug"));
        assert_eq!(json.get("mode").and_then(Value::as_str), Some("hybrid"));
        assert_eq!(json.get("status").and_then(Value::as_str), Some("ok"));
        assert!(json.get("app_root").is_some());
        assert!(json.get("run_id").is_some());
        assert!(json.get("started_at").is_some());
        assert!(json.get("finished_at").is_some());
        assert_eq!(
            json.get("artifacts")
                .and_then(|a| a.as_array())
                .map(|arr| arr.len()),
            Some(2)
        );
        assert!(json.get("events").and_then(Value::as_array).is_some());
        assert!(json.get("measurements").and_then(Value::as_array).is_some());
        // details must be present and non-null
        assert!(
            json.get("details").is_some(),
            "details field must be present"
        );
        assert!(
            !json.get("details").unwrap().is_null(),
            "details must not be null"
        );

        // Verify artifact structure
        let artifacts = json.get("artifacts").unwrap().as_array().unwrap();
        for artifact in artifacts {
            assert!(artifact.get("kind").is_some());
            assert!(artifact.get("path").is_some());
        }
    }

    #[test]
    fn golden_rust_chatbot_header_debug_report_structure() {
        let mut report = Report::new(
            "rust_chatbot",
            "header_debug",
            "hybrid",
            Path::new("/test/app"),
        );
        report.add_artifact("progress_log", "/tmp/out/progress.log", None, Value::Null);
        report.add_artifact("trace_log", "/tmp/out/trace.log", None, Value::Null);
        report.add_artifact("screenshot", "/tmp/out/screenshot.png", None, Value::Null);
        report.push_lifecycle_event("prepare", "prepared");
        report.set_details(json!({
            "provider": "claude",
            "widths": [520],
            "header_height": 140,
        }));
        report.finish_ok();

        let json = serde_json::to_value(&report).unwrap();

        // Verify all required fields
        assert_eq!(
            json.get("scenario").and_then(Value::as_str),
            Some("header_debug")
        );
        assert_eq!(
            json.get("artifacts")
                .and_then(|a| a.as_array())
                .map(|arr| arr.len()),
            Some(3)
        );
        // details must be present and an object
        assert!(
            json.get("details").is_some() && json.get("details").unwrap().is_object(),
            "details must be a non-null object"
        );

        // Verify screenshot artifact has expected structure
        let artifacts = json.get("artifacts").unwrap().as_array().unwrap();
        let screenshot_art = artifacts.iter().find(|a| a.get("kind") == Some(&Value::String("screenshot".to_string())));
        assert!(screenshot_art.is_some(), "screenshot artifact should exist");
    }

    #[test]
    fn golden_rust_chatbot_prompt_debug_report_structure() {
        let mut report = Report::new(
            "rust_chatbot",
            "prompt_debug",
            "hybrid",
            Path::new("/test/app"),
        );
        report.add_artifact("progress_log", "/tmp/out/progress.log", None, Value::Null);
        report.push_measurement(json!({"name": "prompt_latency_ms", "value": 150}));
        report.push_measurement(json!({"name": "upgrade_latency_ms", "value": 45}));
        report.set_details(json!({
            "provider": "codex",
            "session_id": "test-session",
            "prompt": "Hello",
        }));
        report.finish_ok();

        let json = serde_json::to_value(&report).unwrap();

        // Verify scenario and measurements
        assert_eq!(
            json.get("scenario").and_then(Value::as_str),
            Some("prompt_debug")
        );
        assert!(json.get("measurements").and_then(Value::as_array).is_some());
        assert_eq!(
            json.get("measurements")
                .and_then(|m| m.as_array())
                .map(|arr| arr.len()),
            Some(2)
        );

        // Verify measurements have required fields
        let measurements = json.get("measurements").unwrap().as_array().unwrap();
        for m in measurements {
            assert!(m.get("name").is_some(), "measurement should have name");
            assert!(m.get("value").is_some(), "measurement should have value");
        }

        // details must be present
        assert!(
            json.get("details").is_some() && !json.get("details").unwrap().is_null(),
            "details must be present and non-null"
        );
    }

    #[test]
    fn golden_gpui_scroll_matrix_report_structure() {
        let mut report = Report::new(
            "gpui_component_testing",
            "scroll_matrix",
            "startup_driven",
            Path::new("/test/gpui"),
        );
        report.add_artifact("progress_log", "/tmp/out/progress.log", None, Value::Null);
        report.add_artifact("summary_csv", "/tmp/out/summary.csv", None, Value::Null);
        report.push_measurement(json!({"name": "rows_processed", "value": 1000}));
        report.set_details(json!({
            "variants": ["large", "small"],
            "run_ms": 5000,
        }));
        report.finish_ok();

        let json = serde_json::to_value(&report).unwrap();

        // Verify target, scenario, mode
        assert_eq!(
            json.get("target").and_then(Value::as_str),
            Some("gpui_component_testing")
        );
        assert_eq!(
            json.get("scenario").and_then(Value::as_str),
            Some("scroll_matrix")
        );
        assert_eq!(
            json.get("mode").and_then(Value::as_str),
            Some("startup_driven")
        );

        // Verify artifacts structure
        assert_eq!(
            json.get("artifacts")
                .and_then(|a| a.as_array())
                .map(|arr| arr.len()),
            Some(2)
        );

        // Verify details has variants and run_ms
        let details = json.get("details").unwrap();
        assert!(details.get("variants").is_some());
        assert!(details.get("run_ms").is_some());
    }

    #[test]
    fn golden_gpui_scrollbar_trace_report_structure() {
        let mut report = Report::new(
            "gpui_component_testing",
            "scrollbar_trace",
            "startup_driven",
            Path::new("/test/gpui"),
        );
        report.add_artifact("progress_log", "/tmp/out/progress.log", None, Value::Null);
        report.add_artifact("trace_csv", "/tmp/out/trace.csv", None, Value::Null);
        report.set_details(json!({
            "variants": ["default"],
            "run_ms": 3000,
        }));
        report.finish_ok();

        let json = serde_json::to_value(&report).unwrap();

        // Verify scenario and artifacts
        assert_eq!(
            json.get("scenario").and_then(Value::as_str),
            Some("scrollbar_trace")
        );
        assert_eq!(
            json.get("artifacts")
                .and_then(|a| a.as_array())
                .map(|arr| arr.len()),
            Some(2)
        );

        // Verify trace_csv artifact structure
        let artifacts = json.get("artifacts").unwrap().as_array().unwrap();
        let trace_art = artifacts.iter().find(|a| a.get("kind") == Some(&Value::String("trace_csv".to_string())));
        assert!(trace_art.is_some(), "trace_csv artifact should exist");
        assert!(trace_art.unwrap().get("path").is_some());

        // details must be present
        assert!(
            json.get("details").is_some() && json.get("details").unwrap().is_object(),
            "details must be a non-null object"
        );
    }

    #[test]
    fn golden_gpui_conversation_paint_report_structure() {
        let mut report = Report::new(
            "gpui_component_testing",
            "conversation_paint",
            "startup_driven",
            Path::new("/test/gpui"),
        );
        report.add_artifact("progress_log", "/tmp/out/progress.log", None, Value::Null);
        report.add_artifact(
            "conversation_csv",
            "/tmp/out/conversation.csv",
            None,
            Value::Null,
        );
        report.push_measurement(json!({"name": "messages_processed", "value": 50}));
        report.set_details(json!({
            "threads": ["main", "worker"],
            "run_ms": 10000,
        }));
        report.finish_ok();

        let json = serde_json::to_value(&report).unwrap();

        // Verify scenario and measurements
        assert_eq!(
            json.get("scenario").and_then(Value::as_str),
            Some("conversation_paint")
        );
        assert!(json.get("measurements").and_then(Value::as_array).is_some());

        // Verify measurement has name and value
        let measurements = json.get("measurements").unwrap().as_array().unwrap();
        assert!(!measurements.is_empty());
        for m in measurements {
            assert!(m.get("name").is_some());
            assert!(m.get("value").is_some());
        }

        // Verify conversation_csv artifact structure
        let artifacts = json.get("artifacts").unwrap().as_array().unwrap();
        let conv_art = artifacts.iter().find(|a| a.get("kind") == Some(&Value::String("conversation_csv".to_string())));
        assert!(conv_art.is_some(), "conversation_csv artifact should exist");

        // details must be present
        assert!(
            json.get("details").is_some() && !json.get("details").unwrap().is_null(),
            "details must be present and non-null"
        );
    }

    #[test]
    fn normalize_artifact_path_converts_to_absolute() {
        let result = normalize_artifact_path("/tmp/test.log");
        assert!(result.starts_with('/'), "should be absolute path: {}", result);
    }

    #[test]
    fn normalize_artifact_path_resolves_relative() {
        let result = normalize_artifact_path("test.log");
        assert!(result.contains("test.log"), "should contain original name: {}", result);
        assert!(result.starts_with('/'), "should be absolute: {}", result);
    }

    #[test]
    fn normalize_artifact_path_handles_current_dir_prefix() {
        let cwd = std::env::current_dir().unwrap();
        let result = normalize_artifact_path("test.log");
        // Result should be the absolute path of cwd/test.log
        let expected_prefix = cwd.to_string_lossy().trim_end_matches('/').to_string() + "/";
        assert!(
            result.starts_with(&expected_prefix),
            "expected prefix '{}', got: {}",
            expected_prefix,
            result
        );
        assert!(result.ends_with("test.log"), "should end with test.log: {}", result);
    }

    #[test]
    fn report_artifacts_are_normalized_to_absolute_paths() {
        let mut report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/tmp"));

        // Add artifact with relative path - it should be normalized to absolute
        report.add_artifact("progress_log", "relative/path/log.txt", None, Value::Null);

        let json = serde_json::to_value(&report).unwrap();
        let artifact_path = json
            .get("artifacts")
            .unwrap()
            .as_array()
            .unwrap()
            .first()
            .unwrap()
            .get("path")
            .unwrap()
            .as_str()
            .unwrap();

        // The path should be absolute (starting with /)
        assert!(
            artifact_path.starts_with('/'),
            "artifact path should be absolute, got: {}",
            artifact_path
        );
        // And it should contain the filename
        assert!(
            artifact_path.ends_with("relative/path/log.txt") || artifact_path.ends_with("log.txt"),
            "artifact path should contain log.txt: {}",
            artifact_path
        );
    }

    #[test]
    fn validate_artifacts_succeeds_when_all_exist() {
        let mut report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/tmp"));

        // Create temp files for artifacts
        let temp_dir = unique_temp_dir("artifact-validation");
        fs::create_dir_all(&temp_dir).unwrap();

        let progress_path = temp_dir.join("progress.log");
        let trace_path = temp_dir.join("trace.log");
        fs::write(&progress_path, "log content").unwrap();
        fs::write(&trace_path, "trace content").unwrap();

        report.add_artifact("progress_log", progress_path.to_str().unwrap(), None, Value::Null);
        report.add_artifact("trace_log", trace_path.to_str().unwrap(), None, Value::Null);

        let result = validate_artifacts(&report);
        assert!(result.is_ok(), "should succeed when all artifacts exist: {:?}", result);

        // Cleanup
        let _ = fs::remove_file(progress_path);
        let _ = fs::remove_file(trace_path);
        let _ = fs::remove_dir(temp_dir);
    }

    #[test]
    fn validate_artifacts_fails_when_missing() {
        let mut report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/tmp"));
        report.add_artifact("progress_log", "/tmp/nonexistent/path/log.txt", None, Value::Null);
        report.add_artifact("trace_log", "/tmp/also/missing/trace.log", None, Value::Null);

        let result = validate_artifacts(&report);
        assert!(result.is_err(), "should fail when artifacts are missing");

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("missing"), "error should mention 'missing'");
        assert!(err_msg.contains("2"), "error should mention count of missing artifacts");
    }

    #[test]
    fn validate_artifacts_single_missing_reports_count() {
        let mut report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/tmp"));
        report.add_artifact("progress_log", "/tmp/exists.log", None, Value::Null);
        report.add_artifact("trace_log", "/tmp/does_not_exist.log", None, Value::Null);

        let result = validate_artifacts(&report);
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("missing"), "error should mention 'missing'");
        assert!(err_msg.contains("/tmp/does_not_exist.log"), "should include the missing path");
    }

    #[test]
    fn validate_artifacts_empty_report_succeeds() {
        let report = Report::new("rust_chatbot", "debug", "hybrid", Path::new("/tmp"));
        // No artifacts - should succeed
        let result = validate_artifacts(&report);
        assert!(result.is_ok(), "empty artifacts list should pass validation");
    }
}
