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
    #[serde(default, skip_serializing_if = "Value::is_null")]
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
            details: Value::Null,
        }
    }

    pub fn add_artifact(
        &mut self,
        kind: impl Into<String>,
        path: impl Into<String>,
        description: Option<String>,
        metadata: Value,
    ) {
        self.artifacts.push(ArtifactRef {
            kind: kind.into(),
            path: path.into(),
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
        assert_eq!(
            json.get("target").and_then(Value::as_str),
            Some("rust_chatbot")
        );
        assert_eq!(json.get("scenario").and_then(Value::as_str), Some("debug"));
        assert_eq!(json.get("mode").and_then(Value::as_str), Some("hybrid"));
        assert_eq!(json.get("status").and_then(Value::as_str), Some("ok"));
        assert!(json.get("artifacts").and_then(Value::as_array).is_some());
        assert_eq!(
            json.get("artifacts")
                .and_then(|a| a.as_array())
                .map(|arr| arr.len()),
            Some(2)
        );
        assert!(json.get("events").and_then(Value::as_array).is_some());
        assert!(json.get("details").and_then(Value::as_object).is_some());
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
        assert_eq!(
            json.get("scenario").and_then(Value::as_str),
            Some("scrollbar_trace")
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
        assert_eq!(
            json.get("scenario").and_then(Value::as_str),
            Some("conversation_paint")
        );
        assert!(json.get("measurements").and_then(Value::as_array).is_some());
    }
}
