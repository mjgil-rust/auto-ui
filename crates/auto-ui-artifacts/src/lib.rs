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
