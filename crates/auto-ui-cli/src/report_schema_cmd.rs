use anyhow::Result;
use auto_ui_artifacts::report_schema_json;

pub fn run() -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&report_schema_json())?);
    Ok(())
}
