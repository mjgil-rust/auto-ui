use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
            schema_version: "1".to_string(),
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

pub fn write_report(output_dir: &Path, report: &Report) -> Result<PathBuf> {
    let report_path = report.report_path(output_dir);
    fs::write(&report_path, serde_json::to_string_pretty(report)?)
        .with_context(|| format!("failed to write {}", report_path.display()))?;
    Ok(report_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
            schema_version: "1".to_string(),
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
}
