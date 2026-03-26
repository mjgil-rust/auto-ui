use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use auto_ui_artifacts::{write_report, Report};
use auto_ui_core::{
    build_output_dir, expand_path, log_line, normalize_name, repo_root, request_background_launch,
    CompletedRun,
};
use auto_ui_driver_x11 as x11;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const TARGET_ID: &str = "gpui_component_testing";
pub const SCENARIOS: &[&str] = &["scroll_matrix", "scrollbar_trace"];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScrollMatrixConfig {
    pub app_root: Option<String>,
    pub example: String,
    pub variants: Vec<String>,
    pub run_ms: u64,
    pub warmup_ms: u64,
    pub scroll_delay_ms: u64,
    pub scroll_step_px: u64,
    pub output_dir: Option<String>,
    pub capture_window: bool,
    pub settle_ms: u64,
    pub window_title_prefix: String,
}

impl Default for ScrollMatrixConfig {
    fn default() -> Self {
        Self {
            app_root: None,
            example: "llm_chat_story_style_bench_demo".to_string(),
            variants: vec![
                "cached_parent_scroll_markdown".to_string(),
                "cached_markdown".to_string(),
                "cached_parent_scroll_selectable_off".to_string(),
                "plain_text".to_string(),
            ],
            run_ms: 12_000,
            warmup_ms: 1_500,
            scroll_delay_ms: 16,
            scroll_step_px: 40,
            output_dir: None,
            capture_window: false,
            settle_ms: 800,
            window_title_prefix: "Auto UI GPUI Scroll Matrix".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScrollbarTraceConfig {
    pub app_root: Option<String>,
    pub example: String,
    pub run_ms: u64,
    pub warmup_ms: u64,
    pub scroll_delay_ms: u64,
    pub scroll_step_px: u64,
    pub output_dir: Option<String>,
    pub capture_window: bool,
    pub settle_ms: u64,
    pub window_title: String,
}

impl Default for ScrollbarTraceConfig {
    fn default() -> Self {
        Self {
            app_root: None,
            example: "llm_chat_story_style_scrollbar_demo".to_string(),
            run_ms: 4_000,
            warmup_ms: 1_000,
            scroll_delay_ms: 16,
            scroll_step_px: 40,
            output_dir: None,
            capture_window: false,
            settle_ms: 800,
            window_title: "Auto UI GPUI Scrollbar Trace".to_string(),
        }
    }
}

pub fn scenario_names() -> &'static [&'static str] {
    SCENARIOS
}

pub fn validate_named_scenario(scenario: &str, value: &Value) -> Result<()> {
    match normalize_name(scenario).as_str() {
        "scroll_matrix" => {
            scroll_matrix_config_from_scenario(value.clone(), None)?;
            Ok(())
        }
        "scrollbar_trace" => {
            scrollbar_trace_config_from_scenario(value.clone(), None)?;
            Ok(())
        }
        other => bail!("Unsupported gpui scenario {other:?}."),
    }
}

pub fn run_named_scenario(
    scenario: &str,
    value: Value,
    output_override: Option<String>,
) -> Result<CompletedRun> {
    match normalize_name(scenario).as_str() {
        "scroll_matrix" => {
            let config = scroll_matrix_config_from_scenario(value, output_override)?;
            run_scroll_matrix(config)
        }
        "scrollbar_trace" => {
            let config = scrollbar_trace_config_from_scenario(value, output_override)?;
            run_scrollbar_trace(config)
        }
        other => bail!("Unsupported gpui scenario {other:?}."),
    }
}

pub fn run_scroll_matrix(config: ScrollMatrixConfig) -> Result<CompletedRun> {
    auto_ui_core::ensure_display("gpui-component-testing")?;
    let app_root = resolve_app_root(config.app_root.as_deref())?;
    let output_dir = build_output_dir(config.output_dir.as_deref(), "auto-ui-gpui-scroll-matrix")?;
    let progress_path = output_dir.join("progress.log");
    let binary = resolve_example_binary(&app_root, &config.example)?;

    log_line(
        format!("app_root={}", app_root.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("output_dir={}", output_dir.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("example_binary={}", binary.display()),
        Some(&progress_path),
    )?;

    let mut report = Report::new(TARGET_ID, "scroll_matrix", "startup_driven", &app_root);
    report.add_artifact(
        "progress_log",
        progress_path.display().to_string(),
        Some("live progress log".to_string()),
        Value::Null,
    );

    let mut summary_rows = Vec::new();

    for variant in &config.variants {
        let csv_path = output_dir.join(format!("{variant}.frames.csv"));
        let stdout_path = output_dir.join(format!("{variant}.stdout.log"));
        let stderr_path = output_dir.join(format!("{variant}.stderr.log"));
        let screenshot_path = output_dir.join(format!("{variant}.window.png"));
        let window_title = format!("{} - {}", config.window_title_prefix, variant);
        let duration_ms = config.warmup_ms + config.run_ms + 2_000;

        log_line(
            format!("running variant={variant} duration_ms={duration_ms}"),
            Some(&progress_path),
        )?;

        let mut command = Command::new(&binary);
        command.current_dir(&app_root);
        request_background_launch(&mut command);
        command.env("BENCH_VARIANT", variant);
        command.env("BENCH_OUTPUT", &csv_path);
        command.env("BENCH_DURATION_MS", duration_ms.to_string());
        command.env("BENCH_AUTO_SCROLL", "1");
        command.env("BENCH_SCROLL_WARMUP_MS", config.warmup_ms.to_string());
        command.env("BENCH_SCROLL_TICK_MS", config.scroll_delay_ms.to_string());
        command.env("BENCH_SCROLL_STEP_PX", config.scroll_step_px.to_string());
        command.env("BENCH_WINDOW_TITLE", &window_title);

        run_process_with_optional_capture(
            command,
            &stdout_path,
            &stderr_path,
            config.capture_window,
            config.settle_ms,
            Some(window_title.as_str()),
            Some(&screenshot_path),
        )?;

        let summary = summarize_scroll_matrix_csv(&csv_path, config.warmup_ms)?;
        report.push_measurement(json!({
            "variant": variant,
            "samples": summary.samples,
            "avg_frame_ms": summary.avg_frame_ms,
            "p95_frame_ms": summary.p95_frame_ms,
        }));
        report.add_artifact(
            "gpui_csv",
            csv_path.display().to_string(),
            Some(format!("scroll matrix csv for {variant}")),
            json!({ "variant": variant }),
        );
        report.add_artifact(
            "stdout_log",
            stdout_path.display().to_string(),
            Some(format!("stdout for {variant}")),
            json!({ "variant": variant }),
        );
        report.add_artifact(
            "stderr_log",
            stderr_path.display().to_string(),
            Some(format!("stderr for {variant}")),
            json!({ "variant": variant }),
        );
        if config.capture_window && screenshot_path.exists() {
            report.add_artifact(
                "window_screenshot",
                screenshot_path.display().to_string(),
                Some(format!("window screenshot for {variant}")),
                json!({ "variant": variant }),
            );
        }
        summary_rows.push((variant.clone(), summary));
    }

    let summary_md = output_dir.join("summary.md");
    write_scroll_matrix_summary(&summary_md, &summary_rows, &config)?;
    report.add_artifact(
        "summary_markdown",
        summary_md.display().to_string(),
        Some("scroll matrix summary".to_string()),
        Value::Null,
    );
    report.set_details(json!({
        "example": config.example,
        "variants": config.variants,
        "run_ms": config.run_ms,
        "warmup_ms": config.warmup_ms,
        "scroll_delay_ms": config.scroll_delay_ms,
        "scroll_step_px": config.scroll_step_px,
    }));
    report.finish_ok();
    let report_path = write_report(&output_dir, &report)?;

    println!("wrote {}", report_path.display());
    println!("\nartifacts:");
    println!("  progress: {}", progress_path.display());
    println!("  summary: {}", summary_md.display());
    println!("  report: {}", report_path.display());

    Ok(CompletedRun {
        output_dir,
        report_path,
    })
}

pub fn run_scrollbar_trace(config: ScrollbarTraceConfig) -> Result<CompletedRun> {
    auto_ui_core::ensure_display("gpui-component-testing")?;
    let app_root = resolve_app_root(config.app_root.as_deref())?;
    let output_dir =
        build_output_dir(config.output_dir.as_deref(), "auto-ui-gpui-scrollbar-trace")?;
    let progress_path = output_dir.join("progress.log");
    let binary = resolve_example_binary(&app_root, &config.example)?;

    log_line(
        format!("app_root={}", app_root.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("output_dir={}", output_dir.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("example_binary={}", binary.display()),
        Some(&progress_path),
    )?;

    let stdout_path = output_dir.join("scrollbar.stdout.log");
    let stderr_path = output_dir.join("scrollbar.stderr.log");
    let screenshot_path = output_dir.join("scrollbar.window.png");
    let summary_md = output_dir.join("summary.md");
    let duration_ms = config.warmup_ms + config.run_ms + 1_500;

    let mut command = Command::new(&binary);
    command.current_dir(&app_root);
    request_background_launch(&mut command);
    command.env("GPUI_COMPONENT_SCROLLBAR_TRACE", "1");
    command.env("SCROLLBAR_DEMO_AUTO_SCROLL", "1");
    command.env("SCROLLBAR_DEMO_DURATION_MS", duration_ms.to_string());
    command.env(
        "SCROLLBAR_DEMO_SCROLL_WARMUP_MS",
        config.warmup_ms.to_string(),
    );
    command.env(
        "SCROLLBAR_DEMO_SCROLL_TICK_MS",
        config.scroll_delay_ms.to_string(),
    );
    command.env(
        "SCROLLBAR_DEMO_SCROLL_STEP_PX",
        config.scroll_step_px.to_string(),
    );
    command.env("SCROLLBAR_DEMO_WINDOW_TITLE", &config.window_title);

    run_process_with_optional_capture(
        command,
        &stdout_path,
        &stderr_path,
        config.capture_window,
        config.settle_ms,
        Some(config.window_title.as_str()),
        Some(&screenshot_path),
    )?;

    let summary = summarize_scrollbar_stderr(&stderr_path)?;
    write_scrollbar_summary(&summary_md, &summary, &config)?;

    let mut report = Report::new(TARGET_ID, "scrollbar_trace", "startup_driven", &app_root);
    report.add_artifact(
        "progress_log",
        progress_path.display().to_string(),
        Some("live progress log".to_string()),
        Value::Null,
    );
    report.add_artifact(
        "stdout_log",
        stdout_path.display().to_string(),
        Some("scrollbar demo stdout".to_string()),
        Value::Null,
    );
    report.add_artifact(
        "stderr_log",
        stderr_path.display().to_string(),
        Some("scrollbar demo stderr".to_string()),
        Value::Null,
    );
    report.add_artifact(
        "summary_markdown",
        summary_md.display().to_string(),
        Some("scrollbar trace summary".to_string()),
        Value::Null,
    );
    if config.capture_window && screenshot_path.exists() {
        report.add_artifact(
            "window_screenshot",
            screenshot_path.display().to_string(),
            Some("scrollbar trace screenshot".to_string()),
            Value::Null,
        );
    }
    report.push_measurement(json!({
        "trace_lines": summary.total_lines,
        "zero_states": summary.zero_states,
        "nonzero_states": summary.nonzero_states,
    }));
    report.set_details(json!({
        "example": config.example,
        "run_ms": config.run_ms,
        "warmup_ms": config.warmup_ms,
        "scroll_delay_ms": config.scroll_delay_ms,
        "scroll_step_px": config.scroll_step_px,
        "window_title": config.window_title,
        "last_trace_line": summary.last_line,
    }));
    report.finish_ok();
    let report_path = write_report(&output_dir, &report)?;

    println!("wrote {}", report_path.display());
    println!("\nartifacts:");
    println!("  progress: {}", progress_path.display());
    println!("  summary: {}", summary_md.display());
    println!("  report: {}", report_path.display());

    Ok(CompletedRun {
        output_dir,
        report_path,
    })
}

fn resolve_app_root(raw_path: Option<&str>) -> Result<PathBuf> {
    if let Some(raw_path) = raw_path {
        return expand_path(raw_path);
    }

    for env_name in ["GPUI_COMPONENT_TESTING_ROOT", "AUTO_UI_GPUI_APP_ROOT"] {
        if let Ok(value) = std::env::var(env_name) {
            return expand_path(&value);
        }
    }

    let sibling = repo_root()
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .join("gpui-component-testing");
    if sibling.exists() {
        return sibling
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", sibling.display()));
    }

    bail!("Could not resolve the gpui-component-testing app root. Pass --app-root or set GPUI_COMPONENT_TESTING_ROOT.");
}

fn resolve_example_binary(app_root: &Path, example: &str) -> Result<PathBuf> {
    let path = app_root
        .join("target")
        .join("release")
        .join("examples")
        .join(example);
    if path.exists() {
        Ok(path)
    } else {
        bail!(
            "Missing gpui example binary {}. Build it first with the lightweight build path.",
            path.display()
        )
    }
}

fn scroll_matrix_config_from_scenario(
    value: Value,
    output_override: Option<String>,
) -> Result<ScrollMatrixConfig> {
    let scenario: ScrollMatrixScenarioFile = serde_json::from_value(value)?;
    let mut config = ScrollMatrixConfig::default();
    if let Some(app) = scenario.app {
        config.app_root = app.root;
        config.example = app.example.unwrap_or(config.example);
    }
    if let Some(bench) = scenario.bench {
        config.variants = bench.variants.unwrap_or(config.variants);
        config.run_ms = bench.run_ms.unwrap_or(config.run_ms);
        config.warmup_ms = bench.warmup_ms.unwrap_or(config.warmup_ms);
        config.scroll_delay_ms = bench.scroll_delay_ms.unwrap_or(config.scroll_delay_ms);
        config.scroll_step_px = bench.scroll_step_px.unwrap_or(config.scroll_step_px);
        config.window_title_prefix = bench
            .window_title_prefix
            .unwrap_or(config.window_title_prefix);
    }
    if let Some(capture) = scenario.capture {
        config.capture_window = capture.capture_window.unwrap_or(config.capture_window);
        config.settle_ms = capture.settle_ms.unwrap_or(config.settle_ms);
    }
    config.output_dir = output_override.or(scenario.output_dir);
    Ok(config)
}

fn scrollbar_trace_config_from_scenario(
    value: Value,
    output_override: Option<String>,
) -> Result<ScrollbarTraceConfig> {
    let scenario: ScrollbarTraceScenarioFile = serde_json::from_value(value)?;
    let mut config = ScrollbarTraceConfig::default();
    if let Some(app) = scenario.app {
        config.app_root = app.root;
        config.example = app.example.unwrap_or(config.example);
    }
    if let Some(trace) = scenario.trace {
        config.run_ms = trace.run_ms.unwrap_or(config.run_ms);
        config.warmup_ms = trace.warmup_ms.unwrap_or(config.warmup_ms);
        config.scroll_delay_ms = trace.scroll_delay_ms.unwrap_or(config.scroll_delay_ms);
        config.scroll_step_px = trace.scroll_step_px.unwrap_or(config.scroll_step_px);
        config.window_title = trace.window_title.unwrap_or(config.window_title);
    }
    if let Some(capture) = scenario.capture {
        config.capture_window = capture.capture_window.unwrap_or(config.capture_window);
        config.settle_ms = capture.settle_ms.unwrap_or(config.settle_ms);
    }
    config.output_dir = output_override.or(scenario.output_dir);
    Ok(config)
}

#[derive(Deserialize)]
struct ScrollMatrixScenarioFile {
    output_dir: Option<String>,
    app: Option<GpuiApp>,
    bench: Option<ScrollMatrixBench>,
    capture: Option<GpuiCapture>,
}

#[derive(Deserialize)]
struct ScrollbarTraceScenarioFile {
    output_dir: Option<String>,
    app: Option<GpuiApp>,
    trace: Option<ScrollbarTraceSection>,
    capture: Option<GpuiCapture>,
}

#[derive(Deserialize)]
struct GpuiApp {
    root: Option<String>,
    example: Option<String>,
}

#[derive(Deserialize)]
struct ScrollMatrixBench {
    variants: Option<Vec<String>>,
    run_ms: Option<u64>,
    warmup_ms: Option<u64>,
    scroll_delay_ms: Option<u64>,
    scroll_step_px: Option<u64>,
    window_title_prefix: Option<String>,
}

#[derive(Deserialize)]
struct ScrollbarTraceSection {
    run_ms: Option<u64>,
    warmup_ms: Option<u64>,
    scroll_delay_ms: Option<u64>,
    scroll_step_px: Option<u64>,
    window_title: Option<String>,
}

#[derive(Deserialize)]
struct GpuiCapture {
    capture_window: Option<bool>,
    settle_ms: Option<u64>,
}

fn run_process_with_optional_capture(
    mut command: Command,
    stdout_path: &Path,
    stderr_path: &Path,
    capture_window: bool,
    settle_ms: u64,
    window_title: Option<&str>,
    screenshot_path: Option<&Path>,
) -> Result<()> {
    let restore_window_id = x11::get_active_window_id()?;
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| "failed to start gpui process".to_string())?;

    thread::sleep(Duration::from_millis(settle_ms));
    if let Some(window_id) = x11::find_window_id_for_pid(child.id() as i32, Duration::from_secs(2))?
        .or_else(|| {
            window_title.and_then(|title| {
                x11::find_window_id(title, Duration::from_secs(2))
                    .ok()
                    .flatten()
            })
        })
    {
        let _ = x11::background_window(&window_id, restore_window_id.as_deref());
        if capture_window {
            if let Some(screenshot_path) = screenshot_path {
                let _ = x11::capture_window_screenshot(&window_id, screenshot_path);
            }
        }
    }

    let output = child.wait_with_output()?;
    fs::write(stdout_path, &output.stdout)
        .with_context(|| format!("failed to write {}", stdout_path.display()))?;
    fs::write(stderr_path, &output.stderr)
        .with_context(|| format!("failed to write {}", stderr_path.display()))?;
    if !output.status.success() {
        bail!(
            "gpui process failed with status {}. stderr log: {}",
            output.status,
            stderr_path.display()
        );
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct ScrollMatrixSummary {
    samples: usize,
    avg_frame_ms: f64,
    p95_frame_ms: f64,
}

fn summarize_scroll_matrix_csv(path: &Path, warmup_ms: u64) -> Result<ScrollMatrixSummary> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut frame_values = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<_> = line.split(',').collect();
        if cols.len() < 4 {
            continue;
        }
        let timestamp_ms = cols[0].trim().parse::<u64>().unwrap_or(0);
        if timestamp_ms < warmup_ms {
            continue;
        }
        let frame_us = cols[3].trim().parse::<f64>().unwrap_or(0.0);
        frame_values.push(frame_us / 1000.0);
    }

    if frame_values.is_empty() {
        return Ok(ScrollMatrixSummary {
            samples: 0,
            avg_frame_ms: 0.0,
            p95_frame_ms: 0.0,
        });
    }

    let avg_frame_ms = frame_values.iter().sum::<f64>() / frame_values.len() as f64;
    frame_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95_index = ((frame_values.len() - 1) as f64 * 0.95).round() as usize;
    let p95_frame_ms = frame_values[p95_index.min(frame_values.len() - 1)];
    Ok(ScrollMatrixSummary {
        samples: frame_values.len(),
        avg_frame_ms,
        p95_frame_ms,
    })
}

fn write_scroll_matrix_summary(
    path: &Path,
    rows: &[(String, ScrollMatrixSummary)],
    config: &ScrollMatrixConfig,
) -> Result<()> {
    let mut out = String::new();
    out.push_str("# Scroll Matrix Summary\n\n");
    out.push_str(&format!(
        "Run duration: `{}ms` after `{}ms` warmup. Scroll delay: `{}ms`. Step: `{}px`.\n\n",
        config.run_ms, config.warmup_ms, config.scroll_delay_ms, config.scroll_step_px
    ));
    out.push_str("| Variant | Samples | Avg Frame (ms) | P95 Frame (ms) |\n");
    out.push_str("|---|---:|---:|---:|\n");
    for (variant, summary) in rows {
        out.push_str(&format!(
            "| `{}` | {} | {:.2} | {:.2} |\n",
            variant, summary.samples, summary.avg_frame_ms, summary.p95_frame_ms
        ));
    }
    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
}

#[derive(Clone, Debug)]
struct ScrollbarSummary {
    total_lines: usize,
    zero_states: usize,
    nonzero_states: usize,
    last_line: Option<String>,
}

fn summarize_scrollbar_stderr(path: &Path) -> Result<ScrollbarSummary> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let trace_lines: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with("[scrollbar] prepaint"))
        .map(ToOwned::to_owned)
        .collect();
    let zero_states = trace_lines
        .iter()
        .filter(|line| line.contains("states=0"))
        .count();
    let nonzero_states = trace_lines
        .iter()
        .filter(|line| line.contains("states=") && !line.contains("states=0"))
        .count();
    Ok(ScrollbarSummary {
        total_lines: trace_lines.len(),
        zero_states,
        nonzero_states,
        last_line: trace_lines.last().cloned(),
    })
}

fn write_scrollbar_summary(
    path: &Path,
    summary: &ScrollbarSummary,
    config: &ScrollbarTraceConfig,
) -> Result<()> {
    let mut out = String::new();
    out.push_str("# Scrollbar Trace Summary\n\n");
    out.push_str(&format!(
        "Run duration: `{}ms` after `{}ms` warmup. Tick delay: `{}ms`. Step: `{}px`.\n\n",
        config.run_ms, config.warmup_ms, config.scroll_delay_ms, config.scroll_step_px
    ));
    out.push_str(&format!(
        "- Total scrollbar trace lines: `{}`\n",
        summary.total_lines
    ));
    out.push_str(&format!("- `states=0` lines: `{}`\n", summary.zero_states));
    out.push_str(&format!(
        "- `states>0` lines: `{}`\n",
        summary.nonzero_states
    ));
    if let Some(last_line) = &summary.last_line {
        out.push_str(&format!("- Last trace line: `{}`\n", last_line));
    }
    out.push_str("\nInterpretation:\n\n");
    out.push_str("- If `states>0`, the scrollbar logic believes a thumb should be painted.\n");
    out.push_str(
        "- If `states=0` for the whole run, the content likely never overflowed the viewport.\n",
    );
    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn live_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("auto-ui-{name}-{nanos}-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn require_env(name: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this live smoke test"))
    }

    fn require_live_opt_in() {
        assert_eq!(
            std::env::var("AUTO_UI_RUN_LIVE_TESTS").as_deref(),
            Ok("1"),
            "set AUTO_UI_RUN_LIVE_TESTS=1 to run ignored live smoke tests"
        );
    }

    #[test]
    #[ignore = "requires DISPLAY, built gpui examples, and AUTO_UI_RUN_LIVE_TESTS=1"]
    fn live_scroll_matrix_smoke() {
        let _guard = live_test_lock().lock().unwrap();
        require_live_opt_in();
        let output_dir = unique_temp_dir("gpui-scroll-matrix-live");
        let completed = run_scroll_matrix(ScrollMatrixConfig {
            app_root: Some(require_env("AUTO_UI_TEST_GPUI_ROOT")),
            example: "llm_chat_story_style_bench_demo".to_string(),
            variants: vec!["plain_text".to_string()],
            run_ms: 800,
            warmup_ms: 150,
            scroll_delay_ms: 16,
            scroll_step_px: 40,
            output_dir: Some(output_dir.display().to_string()),
            capture_window: false,
            settle_ms: 300,
            window_title_prefix: "Auto UI GPUI Smoke".to_string(),
        })
        .unwrap();
        assert!(completed.report_path.exists());
    }

    #[test]
    #[ignore = "requires DISPLAY, built gpui examples, and AUTO_UI_RUN_LIVE_TESTS=1"]
    fn live_scrollbar_trace_smoke() {
        let _guard = live_test_lock().lock().unwrap();
        require_live_opt_in();
        let output_dir = unique_temp_dir("gpui-scrollbar-live");
        let completed = run_scrollbar_trace(ScrollbarTraceConfig {
            app_root: Some(require_env("AUTO_UI_TEST_GPUI_ROOT")),
            example: "llm_chat_story_style_scrollbar_demo".to_string(),
            run_ms: 800,
            warmup_ms: 150,
            scroll_delay_ms: 16,
            scroll_step_px: 40,
            output_dir: Some(output_dir.display().to_string()),
            capture_window: false,
            settle_ms: 300,
            window_title: "Auto UI GPUI Scrollbar Smoke".to_string(),
        })
        .unwrap();
        assert!(completed.report_path.exists());
    }
}
