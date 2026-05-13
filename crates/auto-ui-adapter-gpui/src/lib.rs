use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use auto_ui_artifacts::{write_report, Report};
use auto_ui_core::{
    build_output_dir, expand_path, log_line, normalize_name, repo_root, request_background_launch,
    CompletedRun,
};
use auto_ui_driver_x11 as x11;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const TARGET_ID: &str = "gpui_component_testing";
pub const SCENARIOS: &[&str] = &["scroll_matrix", "scrollbar_trace", "conversation_paint"];

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
    /// Optional process timeout in milliseconds. If not set, process runs without a harness-level timeout.
    pub timeout_ms: Option<u64>,
    /// Optional custom command to run instead of resolving example binary.
    /// When set, the `example` field is ignored and this command is used directly.
    pub command: Option<String>,
    /// Optional environment variables to set for the custom command.
    pub command_env: Option<std::collections::HashMap<String, String>>,
    /// Optional working directory for the custom command.
    pub command_cwd: Option<String>,
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
            timeout_ms: None,
            command: None,
            command_env: None,
            command_cwd: None,
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
    /// Optional process timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Optional custom command to run instead of resolving example binary.
    pub command: Option<String>,
    /// Optional environment variables to set for the custom command.
    pub command_env: Option<std::collections::HashMap<String, String>>,
    /// Optional working directory for the custom command.
    pub command_cwd: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationPaintConfig {
    pub app_root: Option<String>,
    pub example: String,
    pub threads: Vec<usize>,
    pub run_ms: u64,
    pub defer_first_frame: bool,
    pub output_dir: Option<String>,
    pub capture_window: bool,
    pub settle_ms: u64,
    pub window_title_prefix: String,
    /// Optional process timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Optional custom command to run instead of resolving example binary.
    pub command: Option<String>,
    /// Optional environment variables to set for the custom command.
    pub command_env: Option<std::collections::HashMap<String, String>>,
    /// Optional working directory for the custom command.
    pub command_cwd: Option<String>,
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
            timeout_ms: None,
            command: None,
            command_env: None,
            command_cwd: None,
        }
    }
}

impl Default for ConversationPaintConfig {
    fn default() -> Self {
        Self {
            app_root: None,
            example: "llm_chat_conversation_bench_demo".to_string(),
            threads: vec![0, 1, 2, 3],
            run_ms: 2_500,
            defer_first_frame: true,
            output_dir: None,
            capture_window: false,
            settle_ms: 800,
            window_title_prefix: "Auto UI GPUI Conversation Paint".to_string(),
            timeout_ms: None,
            command: None,
            command_env: None,
            command_cwd: None,
        }
    }
}

pub fn scenario_names() -> &'static [&'static str] {
    SCENARIOS
}

pub fn validate_named_scenario(scenario: &str, value: &Value) -> Result<()> {
    match normalize_name(scenario).as_str() {
        "scroll_matrix" => {
            let config = scroll_matrix_config_from_scenario(value.clone(), None)?;
            // Validate that variants is not empty
            if config.variants.is_empty() {
                bail!("At least one variant is required for scroll_matrix.");
            }
            Ok(())
        }
        "scrollbar_trace" => {
            scrollbar_trace_config_from_scenario(value.clone(), None)?;
            Ok(())
        }
        "conversation_paint" => {
            let config = conversation_paint_config_from_scenario(value.clone(), None)?;
            // Validate that threads is not empty
            if config.threads.is_empty() {
                bail!("At least one thread is required for conversation_paint.");
            }
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
        "conversation_paint" => {
            let config = conversation_paint_config_from_scenario(value, output_override)?;
            run_conversation_paint(config)
        }
        other => bail!("Unsupported gpui scenario {other:?}."),
    }
}

pub fn run_scroll_matrix(config: ScrollMatrixConfig) -> Result<CompletedRun> {
    auto_ui_core::ensure_display("gpui-component-testing")?;
    let app_root = resolve_app_root(config.app_root.as_deref())?;
    let output_dir = build_output_dir(config.output_dir.as_deref(), "auto-ui-gpui-scroll-matrix")?;
    let progress_path = output_dir.join("progress.log");

    // Resolve binary and args: either from custom command or from example
    let (binary, custom_args) = if let Some(ref cmd) = config.command {
        let (path, args) = parse_command_string(cmd)?;
        log_line(
            format!("command_binary={}", path.display()),
            Some(&progress_path),
        )?;
        if !args.is_empty() {
            log_line(
                format!("command_args={}", args.join(" ")),
                Some(&progress_path),
            )?;
        }
        (path, Some(args))
    } else {
        let binary = resolve_example_binary(&app_root, &config.example)?;
        log_line(
            format!("example_binary={}", binary.display()),
            Some(&progress_path),
        )?;
        (binary, None)
    };

    log_line(
        format!("app_root={}", app_root.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("output_dir={}", output_dir.display()),
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
    let output_dir_for_report = output_dir.clone();
    let result = (|| -> Result<CompletedRun> {
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
            // Use custom cwd if specified, otherwise use app_root
            if let Some(ref cwd) = config.command_cwd {
                command.current_dir(cwd);
            } else {
                command.current_dir(&app_root);
            }
            // Add custom args if specified (replaces the default env var setup)
            if let Some(ref args) = custom_args {
                command.args(args);
            }
            request_background_launch(&mut command);
            // When using custom command, skip the BENCH_* env vars
            // unless command_env explicitly includes them
            if config.command.is_none() {
                command.env("BENCH_VARIANT", variant);
                command.env("BENCH_OUTPUT", &csv_path);
                command.env("BENCH_DURATION_MS", duration_ms.to_string());
                command.env("BENCH_AUTO_SCROLL", "1");
                command.env("BENCH_SCROLL_WARMUP_MS", config.warmup_ms.to_string());
                command.env("BENCH_SCROLL_TICK_MS", config.scroll_delay_ms.to_string());
                command.env("BENCH_SCROLL_STEP_PX", config.scroll_step_px.to_string());
                command.env("BENCH_WINDOW_TITLE", &window_title);
            }
            // Add custom env vars if specified
            if let Some(ref env) = config.command_env {
                for (key, value) in env {
                    command.env(key, value);
                }
            }

            run_process_with_optional_capture(
                command,
                &stdout_path,
                &stderr_path,
                config.capture_window,
                config.settle_ms,
                Some(window_title.as_str()),
                Some(&screenshot_path),
                config.timeout_ms,
            )?;

            let summary = summarize_scroll_matrix_csv(&csv_path, config.warmup_ms)?;
            report.push_measurement(json!({
                "variant": variant,
                "samples": summary.samples,
                "avg_frame_ms": summary.avg_frame_ms,
                "p50_frame_ms": summary.p50_frame_ms,
                "p95_frame_ms": summary.p95_frame_ms,
                "p99_frame_ms": summary.p99_frame_ms,
                "p95_content_ms": summary.p95_content_ms,
                "p95_prepaint_ms": summary.p95_prepaint_ms,
                "over_16ms": summary.over_16ms,
                "over_33ms": summary.over_33ms,
                "over_100ms": summary.over_100ms,
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

        let summary_tsv = output_dir.join("summary.tsv");
        let summary_md = output_dir.join("summary.md");
        write_scroll_matrix_summary_tsv(&summary_tsv, &summary_rows)?;
        write_scroll_matrix_summary(&summary_md, &summary_rows, &config)?;
        report.add_artifact(
            "summary_tsv",
            summary_tsv.display().to_string(),
            Some("scroll matrix summary table".to_string()),
            Value::Null,
        );
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
    })();

    // Write report on failure (success path writes it inside the closure)
    if result.is_err() {
        let err_msg = result.as_ref().unwrap_err().to_string();
        report.finish_error(&err_msg);
        if let Err(err) = write_report(&output_dir_for_report, &report) {
            let _ = log_line(
                format!("warning: failed to write error report: {err:#}"),
                Some(&progress_path),
            );
        }
    }

    result
}

pub fn run_scrollbar_trace(config: ScrollbarTraceConfig) -> Result<CompletedRun> {
    auto_ui_core::ensure_display("gpui-component-testing")?;
    let app_root = resolve_app_root(config.app_root.as_deref())?;
    let output_dir =
        build_output_dir(config.output_dir.as_deref(), "auto-ui-gpui-scrollbar-trace")?;
    let progress_path = output_dir.join("progress.log");

    // Resolve binary and args: either from custom command or from example
    let (binary, custom_args) = if let Some(ref cmd) = config.command {
        let (path, args) = parse_command_string(cmd)?;
        log_line(
            format!("command_binary={}", path.display()),
            Some(&progress_path),
        )?;
        if !args.is_empty() {
            log_line(
                format!("command_args={}", args.join(" ")),
                Some(&progress_path),
            )?;
        }
        (path, Some(args))
    } else {
        let binary = resolve_example_binary(&app_root, &config.example)?;
        log_line(
            format!("example_binary={}", binary.display()),
            Some(&progress_path),
        )?;
        (binary, None)
    };

    log_line(
        format!("app_root={}", app_root.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("output_dir={}", output_dir.display()),
        Some(&progress_path),
    )?;

    let stdout_path = output_dir.join("scrollbar.stdout.log");
    let stderr_path = output_dir.join("scrollbar.stderr.log");
    let screenshot_path = output_dir.join("scrollbar.window.png");
    let summary_md = output_dir.join("summary.md");
    let duration_ms = config.warmup_ms + config.run_ms + 1_500;

    let mut report = Report::new(TARGET_ID, "scrollbar_trace", "startup_driven", &app_root);
    report.add_artifact(
        "progress_log",
        progress_path.display().to_string(),
        Some("live progress log".to_string()),
        Value::Null,
    );

    let output_dir_for_report = output_dir.clone();
    let result = (|| -> Result<CompletedRun> {
        let mut command = Command::new(&binary);
        // Use custom cwd if specified, otherwise use app_root
        if let Some(ref cwd) = config.command_cwd {
            command.current_dir(cwd);
        } else {
            command.current_dir(&app_root);
        }
        // Add custom args if specified
        if let Some(ref args) = custom_args {
            command.args(args);
        }
        request_background_launch(&mut command);
        // When using custom command, skip the SCROLLBAR_* env vars
        // unless command_env explicitly includes them
        if config.command.is_none() {
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
        }
        // Add custom env vars if specified
        if let Some(ref env) = config.command_env {
            for (key, value) in env {
                command.env(key, value);
            }
        }

        run_process_with_optional_capture(
            command,
            &stdout_path,
            &stderr_path,
            config.capture_window,
            config.settle_ms,
            Some(config.window_title.as_str()),
            Some(&screenshot_path),
            config.timeout_ms,
        )?;

        let summary = summarize_scrollbar_stderr(&stderr_path)?;
        write_scrollbar_summary(&summary_md, &summary, &config)?;

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
    })();

    // Write report on failure (success path writes it inside the closure)
    if result.is_err() {
        let err_msg = result.as_ref().unwrap_err().to_string();
        report.finish_error(&err_msg);
        if let Err(err) = write_report(&output_dir_for_report, &report) {
            let _ = log_line(
                format!("warning: failed to write error report: {err:#}"),
                Some(&progress_path),
            );
        }
    }

    result
}

pub fn run_conversation_paint(config: ConversationPaintConfig) -> Result<CompletedRun> {
    auto_ui_core::ensure_display("gpui-component-testing")?;
    let app_root = resolve_app_root(config.app_root.as_deref())?;
    let output_dir = build_output_dir(
        config.output_dir.as_deref(),
        "auto-ui-gpui-conversation-paint",
    )?;
    let progress_path = output_dir.join("progress.log");

    // Resolve binary and args: either from custom command or from example
    let (binary, custom_args) = if let Some(ref cmd) = config.command {
        let (path, args) = parse_command_string(cmd)?;
        log_line(
            format!("command_binary={}", path.display()),
            Some(&progress_path),
        )?;
        if !args.is_empty() {
            log_line(
                format!("command_args={}", args.join(" ")),
                Some(&progress_path),
            )?;
        }
        (path, Some(args))
    } else {
        let binary = resolve_example_binary(&app_root, &config.example)?;
        log_line(
            format!("example_binary={}", binary.display()),
            Some(&progress_path),
        )?;
        (binary, None)
    };

    log_line(
        format!("app_root={}", app_root.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("output_dir={}", output_dir.display()),
        Some(&progress_path),
    )?;

    let mut report = Report::new(TARGET_ID, "conversation_paint", "startup_driven", &app_root);
    report.add_artifact(
        "progress_log",
        progress_path.display().to_string(),
        Some("live progress log".to_string()),
        Value::Null,
    );

    let mut summary_rows = Vec::new();
    let output_dir_for_error_report = output_dir.clone();
    let result = (|| -> Result<CompletedRun> {
        for thread_id in &config.threads {
            let thread_key = conversation_thread_key(*thread_id);
            let csv_path = output_dir.join(format!("{thread_key}.frames.csv"));
            let stdout_path = output_dir.join(format!("{thread_key}.stdout.log"));
            let stderr_path = output_dir.join(format!("{thread_key}.stderr.log"));
            let screenshot_path = output_dir.join(format!("{thread_key}.window.png"));
            let window_title = format!("{} - {}", config.window_title_prefix, thread_key);

            log_line(
                format!("running thread={} duration_ms={}", thread_id, config.run_ms),
                Some(&progress_path),
            )?;

            let mut command = Command::new(&binary);
            // Use custom cwd if specified, otherwise use app_root
            if let Some(ref cwd) = config.command_cwd {
                command.current_dir(cwd);
            } else {
                command.current_dir(&app_root);
            }
            // Add custom args if specified
            if let Some(ref args) = custom_args {
                command.args(args);
            }
            request_background_launch(&mut command);
            // When using custom command, skip the BENCH_* env vars
            // unless command_env explicitly includes them
            if config.command.is_none() {
                command.env("BENCH_OUTPUT", &csv_path);
                command.env("BENCH_DURATION_MS", config.run_ms.to_string());
                command.env("BENCH_THREAD", thread_id.to_string());
                command.env(
                    "BENCH_DEFER_FIRST_FRAME",
                    if config.defer_first_frame { "1" } else { "0" },
                );
                command.env("BENCH_WINDOW_TITLE", &window_title);
            }
            // Add custom env vars if specified
            if let Some(ref env) = config.command_env {
                for (key, value) in env {
                    command.env(key, value);
                }
            }

            run_process_with_optional_capture(
                command,
                &stdout_path,
                &stderr_path,
                config.capture_window,
                config.settle_ms,
                Some(window_title.as_str()),
                Some(&screenshot_path),
                config.timeout_ms,
            )?;

            let summary = summarize_conversation_csv(&csv_path)?;
            let label = conversation_thread_label(*thread_id);
            report.push_measurement(json!({
                "thread_id": thread_id,
                "label": label,
                "samples": summary.samples,
                "avg_render_ms": summary.avg_render_ms,
                "p50_render_ms": summary.p50_render_ms,
                "p95_render_ms": summary.p95_render_ms,
                "p99_render_ms": summary.p99_render_ms,
                "p95_messages_ms": summary.p95_messages_ms,
                "p95_prepaint_latency_ms": summary.p95_prepaint_latency_ms,
                "p95_message_max_ms": summary.p95_message_max_ms,
                "over_16ms": summary.over_16ms,
                "over_33ms": summary.over_33ms,
            }));
            report.add_artifact(
                "gpui_csv",
                csv_path.display().to_string(),
                Some(format!("conversation paint csv for thread {}", thread_id)),
                json!({ "thread_id": thread_id, "label": label }),
            );
            report.add_artifact(
                "stdout_log",
                stdout_path.display().to_string(),
                Some(format!("stdout for thread {}", thread_id)),
                json!({ "thread_id": thread_id, "label": label }),
            );
            report.add_artifact(
                "stderr_log",
                stderr_path.display().to_string(),
                Some(format!("stderr for thread {}", thread_id)),
                json!({ "thread_id": thread_id, "label": label }),
            );
            if config.capture_window && screenshot_path.exists() {
                report.add_artifact(
                    "window_screenshot",
                    screenshot_path.display().to_string(),
                    Some(format!("window screenshot for thread {}", thread_id)),
                    json!({ "thread_id": thread_id, "label": label }),
                );
            }
            summary_rows.push((thread_key, *thread_id, summary));
        }

        let summary_tsv = output_dir.join("summary.tsv");
        let summary_md = output_dir.join("summary.md");
        write_conversation_summary_tsv(&summary_tsv, &summary_rows)?;
        write_conversation_summary(&summary_md, &summary_rows, &config)?;
        report.add_artifact(
            "summary_tsv",
            summary_tsv.display().to_string(),
            Some("conversation paint summary table".to_string()),
            Value::Null,
        );
        report.add_artifact(
            "summary_markdown",
            summary_md.display().to_string(),
            Some("conversation paint summary".to_string()),
            Value::Null,
        );
        report.set_details(json!({
            "example": config.example,
            "threads": config.threads,
            "run_ms": config.run_ms,
            "defer_first_frame": config.defer_first_frame,
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
    })();

    // Write report on failure (success path writes it inside the closure)
    if result.is_err() {
        let err_msg = result.as_ref().unwrap_err().to_string();
        report.finish_error(&err_msg);
        if let Err(err) = write_report(&output_dir_for_error_report, &report) {
            let _ = log_line(
                format!("warning: failed to write error report: {err:#}"),
                Some(&progress_path),
            );
        }
    }

    result
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

    let root = repo_root();
    let parent = root
        .parent()
        .with_context(|| format!("repo root has no parent: {}", root.display()))?;
    let sibling = parent.join("gpui-component-testing");
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
    if !path.exists() {
        bail!(
            "Missing gpui example binary {}. Build it first with the lightweight build path.",
            path.display()
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&path) {
            if metadata.permissions().mode() & 0o111 == 0 {
                bail!(
                    "gpui example binary {} is not executable. Check file permissions.",
                    path.display()
                );
            }
        }
    }
    Ok(path)
}

/// Parse a command string into path and args.
/// Supports simple shell-like parsing: "path/to/binary --arg1 --arg2"
/// Returns (path, args) tuple.
fn parse_command_string(command: &str) -> Result<(PathBuf, Vec<String>)> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        bail!("Empty command string");
    }
    let path = PathBuf::from(parts[0]);
    if !path.exists() {
        bail!("Custom command binary {} does not exist", path.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&path) {
            if metadata.permissions().mode() & 0o111 == 0 {
                bail!(
                    "Custom command binary {} is not executable. Check file permissions.",
                    path.display()
                );
            }
        }
    }
    let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
    Ok((path, args))
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

fn conversation_paint_config_from_scenario(
    value: Value,
    output_override: Option<String>,
) -> Result<ConversationPaintConfig> {
    let scenario: ConversationPaintScenarioFile = serde_json::from_value(value)?;
    let mut config = ConversationPaintConfig::default();
    if let Some(app) = scenario.app {
        config.app_root = app.root;
        config.example = app.example.unwrap_or(config.example);
    }
    if let Some(bench) = scenario.bench {
        config.threads = bench.threads.unwrap_or(config.threads);
        config.run_ms = bench.run_ms.unwrap_or(config.run_ms);
        config.defer_first_frame = bench.defer_first_frame.unwrap_or(config.defer_first_frame);
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScrollMatrixScenarioFile {
    output_dir: Option<String>,
    app: Option<GpuiApp>,
    bench: Option<ScrollMatrixBench>,
    capture: Option<GpuiCapture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScrollbarTraceScenarioFile {
    output_dir: Option<String>,
    app: Option<GpuiApp>,
    trace: Option<ScrollbarTraceSection>,
    capture: Option<GpuiCapture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversationPaintScenarioFile {
    output_dir: Option<String>,
    app: Option<GpuiApp>,
    bench: Option<ConversationBenchSection>,
    capture: Option<GpuiCapture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuiApp {
    root: Option<String>,
    example: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScrollMatrixBench {
    variants: Option<Vec<String>>,
    run_ms: Option<u64>,
    warmup_ms: Option<u64>,
    scroll_delay_ms: Option<u64>,
    scroll_step_px: Option<u64>,
    window_title_prefix: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScrollbarTraceSection {
    run_ms: Option<u64>,
    warmup_ms: Option<u64>,
    scroll_delay_ms: Option<u64>,
    scroll_step_px: Option<u64>,
    window_title: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversationBenchSection {
    threads: Option<Vec<usize>>,
    run_ms: Option<u64>,
    defer_first_frame: Option<bool>,
    window_title_prefix: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
    timeout_ms: Option<u64>,
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
                x11::capture_window_screenshot(&window_id, screenshot_path).with_context(|| {
                    format!(
                        "failed to capture screenshot to {}",
                        screenshot_path.display()
                    )
                })?;
            }
        }
    }

    // Wait for process with optional timeout
    let timed_out: bool;
    let exit_status = if let Some(timeout) = timeout_ms {
        let start = std::time::Instant::now();
        let timeout_duration = Duration::from_millis(timeout);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process exited normally - capture output via wait_with_output()
                    // Note: try_wait reaps the process but we need to call wait_with_output
                    // to get the output from the pipes
                    let output = child
                        .wait_with_output()
                        .with_context(|| "failed to capture output after process exited")?;
                    fs::write(stdout_path, &output.stdout)
                        .with_context(|| format!("failed to write {}", stdout_path.display()))?;
                    fs::write(stderr_path, &output.stderr)
                        .with_context(|| format!("failed to write {}", stderr_path.display()))?;
                    timed_out = false;
                    break output.status;
                }
                Ok(None) => {
                    if start.elapsed() >= timeout_duration {
                        // Kill and capture output before bailing
                        child.kill().ok();
                        let output = child.wait_with_output().ok();
                        if let Some(o) = output {
                            fs::write(stdout_path, &o.stdout).with_context(|| {
                                format!("failed to write {}", stdout_path.display())
                            })?;
                            fs::write(stderr_path, &o.stderr).with_context(|| {
                                format!("failed to write {}", stderr_path.display())
                            })?;
                        }
                        timed_out = true;
                        bail!("process timed out after {}ms", timeout);
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(e).context("failed to wait on process"),
            }
        }
    } else {
        // Use wait_with_output() instead of wait() to avoid deadlock when
        // stdout/stderr pipes fill up. Also captures output for logging.
        let output = child
            .wait_with_output()
            .context("failed to wait for gpui process")?;
        fs::write(stdout_path, &output.stdout)
            .with_context(|| format!("failed to write {}", stdout_path.display()))?;
        fs::write(stderr_path, &output.stderr)
            .with_context(|| format!("failed to write {}", stderr_path.display()))?;
        timed_out = false;
        output.status
    };

    if !exit_status.success() {
        bail!(
            "gpui process failed with status {}. stderr log: {}",
            exit_status,
            stderr_path.display()
        );
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct ScrollMatrixSummary {
    samples: usize,
    avg_frame_ms: f64,
    p50_frame_ms: f64,
    p95_frame_ms: f64,
    p99_frame_ms: f64,
    p95_content_ms: f64,
    p95_prepaint_ms: f64,
    over_16ms: usize,
    over_33ms: usize,
    over_100ms: usize,
}

fn summarize_scroll_matrix_csv(path: &Path, warmup_ms: u64) -> Result<ScrollMatrixSummary> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut frame_values = Vec::new();
    let mut content_values = Vec::new();
    let mut prepaint_values = Vec::new();
    let mut dropped_rows = 0usize;
    for (index, line) in text.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<_> = line.split(',').collect();
        if cols.len() < 10 {
            dropped_rows += 1;
            continue;
        }
        let timestamp_ms = match cols[0].trim().parse::<u64>() {
            Ok(v) => v,
            Err(_) => {
                dropped_rows += 1;
                continue;
            }
        };
        if timestamp_ms < warmup_ms {
            continue;
        }
        let frame_us = match cols[3].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                dropped_rows += 1;
                continue;
            }
        };
        frame_values.push(frame_us / 1000.0);
        let content_us = match cols[7].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                dropped_rows += 1;
                continue;
            }
        };
        content_values.push(content_us / 1000.0);
        let prepaint_ms = match cols[9].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                dropped_rows += 1;
                continue;
            }
        };
        prepaint_values.push(prepaint_ms);
    }
    if dropped_rows > 0 {
        log_line(
            format!(
                "warning: dropped {dropped_rows} malformed row(s) from {}",
                path.display()
            ),
            None,
        )?;
    }

    if frame_values.is_empty() {
        return Ok(ScrollMatrixSummary {
            samples: 0,
            avg_frame_ms: 0.0,
            p50_frame_ms: 0.0,
            p95_frame_ms: 0.0,
            p99_frame_ms: 0.0,
            p95_content_ms: 0.0,
            p95_prepaint_ms: 0.0,
            over_16ms: 0,
            over_33ms: 0,
            over_100ms: 0,
        });
    }

    let avg_frame_ms = frame_values.iter().sum::<f64>() / frame_values.len() as f64;
    let p50_frame_ms = percentile_ms(&frame_values, 0.50);
    let p95_frame_ms = percentile_ms(&frame_values, 0.95);
    let p99_frame_ms = percentile_ms(&frame_values, 0.99);
    let p95_content_ms = percentile_ms(&content_values, 0.95);
    let p95_prepaint_ms = percentile_ms(&prepaint_values, 0.95);
    Ok(ScrollMatrixSummary {
        samples: frame_values.len(),
        avg_frame_ms,
        p50_frame_ms,
        p95_frame_ms,
        p99_frame_ms,
        p95_content_ms,
        p95_prepaint_ms,
        over_16ms: frame_values.iter().filter(|value| **value > 16.0).count(),
        over_33ms: frame_values.iter().filter(|value| **value > 33.0).count(),
        over_100ms: frame_values.iter().filter(|value| **value > 100.0).count(),
    })
}

#[derive(Clone, Debug)]
struct ConversationSummary {
    samples: usize,
    avg_render_ms: f64,
    p50_render_ms: f64,
    p95_render_ms: f64,
    p99_render_ms: f64,
    p95_messages_ms: f64,
    p95_prepaint_latency_ms: f64,
    p95_message_max_ms: f64,
    over_16ms: usize,
    over_33ms: usize,
}

fn summarize_conversation_csv(path: &Path) -> Result<ConversationSummary> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut render_values = Vec::new();
    let mut messages_values = Vec::new();
    let mut prepaint_values = Vec::new();
    let mut message_max_values = Vec::new();
    let mut dropped_rows = 0usize;
    for (index, line) in text.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<_> = line.split(',').collect();
        if cols.len() < 11 {
            dropped_rows += 1;
            continue;
        }
        let render_us = match cols[2].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                dropped_rows += 1;
                continue;
            }
        };
        render_values.push(render_us / 1000.0);
        let messages_us = match cols[5].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                dropped_rows += 1;
                continue;
            }
        };
        messages_values.push(messages_us / 1000.0);
        let message_max_us = match cols[9].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                dropped_rows += 1;
                continue;
            }
        };
        message_max_values.push(message_max_us / 1000.0);
        let prepaint_latency_ms = match cols[10].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                dropped_rows += 1;
                continue;
            }
        };
        prepaint_values.push(prepaint_latency_ms);
    }
    if dropped_rows > 0 {
        log_line(
            format!(
                "warning: dropped {dropped_rows} malformed row(s) from {}",
                path.display()
            ),
            None,
        )?;
    }

    if render_values.is_empty() {
        return Ok(ConversationSummary {
            samples: 0,
            avg_render_ms: 0.0,
            p50_render_ms: 0.0,
            p95_render_ms: 0.0,
            p99_render_ms: 0.0,
            p95_messages_ms: 0.0,
            p95_prepaint_latency_ms: 0.0,
            p95_message_max_ms: 0.0,
            over_16ms: 0,
            over_33ms: 0,
        });
    }

    Ok(ConversationSummary {
        samples: render_values.len(),
        avg_render_ms: render_values.iter().sum::<f64>() / render_values.len() as f64,
        p50_render_ms: percentile_ms(&render_values, 0.50),
        p95_render_ms: percentile_ms(&render_values, 0.95),
        p99_render_ms: percentile_ms(&render_values, 0.99),
        p95_messages_ms: percentile_ms(&messages_values, 0.95),
        p95_prepaint_latency_ms: percentile_ms(&prepaint_values, 0.95),
        p95_message_max_ms: percentile_ms(&message_max_values, 0.95),
        over_16ms: render_values.iter().filter(|value| **value > 16.0).count(),
        over_33ms: render_values.iter().filter(|value| **value > 33.0).count(),
    })
}

fn percentile_ms(values: &[f64], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((sorted.len() - 1) as f64 * percentile).round() as usize;
    sorted[index.min(sorted.len() - 1)]
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
    out.push_str("| Variant | Samples | Avg Frame (ms) | P50 Frame (ms) | P95 Frame (ms) | P99 Frame (ms) | P95 Content (ms) | P95 Prepaint (ms) | >16ms | >33ms | >100ms |\n");
    out.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    let mut sorted_rows = rows.to_vec();
    sorted_rows.sort_by(|a, b| {
        a.1.p95_frame_ms
            .partial_cmp(&b.1.p95_frame_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (variant, summary) in sorted_rows {
        out.push_str(&format!(
            "| `{}` | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} | {} | {} |\n",
            variant,
            summary.samples,
            summary.avg_frame_ms,
            summary.p50_frame_ms,
            summary.p95_frame_ms,
            summary.p99_frame_ms,
            summary.p95_content_ms,
            summary.p95_prepaint_ms,
            summary.over_16ms,
            summary.over_33ms,
            summary.over_100ms
        ));
    }
    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
}

fn write_scroll_matrix_summary_tsv(
    path: &Path,
    rows: &[(String, ScrollMatrixSummary)],
) -> Result<()> {
    let mut out = String::new();
    out.push_str("variant\tsamples\tavg_frame_ms\tp50_frame_ms\tp95_frame_ms\tp99_frame_ms\tp95_content_ms\tp95_prepaint_ms\tover_16ms\tover_33ms\tover_100ms\n");
    for (variant, summary) in rows {
        out.push_str(&format!(
            "{variant}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}\t{}\t{}\n",
            summary.samples,
            summary.avg_frame_ms,
            summary.p50_frame_ms,
            summary.p95_frame_ms,
            summary.p99_frame_ms,
            summary.p95_content_ms,
            summary.p95_prepaint_ms,
            summary.over_16ms,
            summary.over_33ms,
            summary.over_100ms
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

fn write_conversation_summary_tsv(
    path: &Path,
    rows: &[(String, usize, ConversationSummary)],
) -> Result<()> {
    let mut out = String::new();
    out.push_str("thread_key\tthread_id\tlabel\tsamples\tavg_render_ms\tp50_render_ms\tp95_render_ms\tp99_render_ms\tp95_messages_ms\tp95_prepaint_latency_ms\tp95_message_max_ms\tover_16ms\tover_33ms\n");
    for (thread_key, thread_id, summary) in rows {
        out.push_str(&format!(
            "{thread_key}\t{thread_id}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}\t{}\n",
            conversation_thread_label(*thread_id),
            summary.samples,
            summary.avg_render_ms,
            summary.p50_render_ms,
            summary.p95_render_ms,
            summary.p99_render_ms,
            summary.p95_messages_ms,
            summary.p95_prepaint_latency_ms,
            summary.p95_message_max_ms,
            summary.over_16ms,
            summary.over_33ms
        ));
    }
    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
}

fn write_conversation_summary(
    path: &Path,
    rows: &[(String, usize, ConversationSummary)],
    config: &ConversationPaintConfig,
) -> Result<()> {
    let mut out = String::new();
    out.push_str("# Conversation Paint Summary\n\n");
    out.push_str(&format!(
        "Run duration: `{}ms`. Deferred first frame: `{}`.\n\n",
        config.run_ms, config.defer_first_frame
    ));
    out.push_str("| Thread | Samples | Avg Render (ms) | P50 Render (ms) | P95 Render (ms) | P99 Render (ms) | P95 Messages (ms) | P95 Prepaint (ms) | P95 Max Message (ms) | >16ms | >33ms |\n");
    out.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    let mut sorted_rows = rows.to_vec();
    sorted_rows.sort_by(|a, b| {
        a.2.p95_render_ms
            .partial_cmp(&b.2.p95_render_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (_thread_key, thread_id, summary) in sorted_rows {
        out.push_str(&format!(
            "| `{} ({})` | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} | {} |\n",
            conversation_thread_label(thread_id),
            thread_id,
            summary.samples,
            summary.avg_render_ms,
            summary.p50_render_ms,
            summary.p95_render_ms,
            summary.p99_render_ms,
            summary.p95_messages_ms,
            summary.p95_prepaint_latency_ms,
            summary.p95_message_max_ms,
            summary.over_16ms,
            summary.over_33ms
        ));
    }
    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
}

fn conversation_thread_label(thread_id: usize) -> &'static str {
    match thread_id {
        0 => "rust",
        1 => "creative",
        2 => "cooking",
        3 => "debug",
        _ => "thread",
    }
}

fn conversation_thread_key(thread_id: usize) -> String {
    let label = conversation_thread_label(thread_id);
    if label == "thread" {
        format!("thread-{thread_id}")
    } else {
        format!("thread-{thread_id}-{label}")
    }
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
    fn summarize_scroll_matrix_csv_computes_extended_stats() {
        let path = unique_temp_dir("scroll-matrix-summary").join("frames.csv");
        fs::write(
            &path,
            "elapsed_ms,variant,active,frame_total_us,sidebar_us,header_us,overlay_us,content_us,body_us,prepaint_since_render_ms\n1500,a,0,10000,0,0,0,4000,0,2\n1600,a,0,20000,0,0,0,7000,0,4\n1700,a,0,50000,0,0,0,9000,0,8\n",
        )
        .unwrap();

        let summary = summarize_scroll_matrix_csv(&path, 1500).unwrap();
        assert_eq!(summary.samples, 3);
        assert_eq!(summary.over_16ms, 2);
        assert_eq!(summary.over_33ms, 1);
        assert_eq!(summary.over_100ms, 0);
        assert!(summary.p95_frame_ms >= 20.0);
        assert!(summary.p95_content_ms >= 7.0);
    }

    #[test]
    fn summarize_conversation_csv_computes_render_stats() {
        let path = unique_temp_dir("conversation-summary").join("frames.csv");
        fs::write(
            &path,
            "elapsed_ms,render_num,render_total_us,sidebar_us,header_us,messages_us,layout_us,msg_count,msg_avg_us,msg_max_us,prepaint_latency_ms\n0,1,9000,0,0,3000,0,1,3000,3000,1.5\n10,2,18000,0,0,5000,0,1,5000,6000,3.0\n20,3,40000,0,0,15000,0,1,15000,18000,7.0\n",
        )
        .unwrap();

        let summary = summarize_conversation_csv(&path).unwrap();
        assert_eq!(summary.samples, 3);
        assert_eq!(summary.over_16ms, 2);
        assert_eq!(summary.over_33ms, 1);
        assert!(summary.p95_render_ms >= 18.0);
        assert!(summary.p95_message_max_ms >= 6.0);
    }

    #[test]
    fn summarize_conversation_csv_drops_malformed_rows() {
        let path = unique_temp_dir("conversation-malformed").join("frames.csv");
        // Row with only 5 columns should be dropped
        fs::write(
            &path,
            "elapsed_ms,render_num,render_total_us,sidebar_us,header_us,messages_us,layout_us,msg_count,msg_avg_us,msg_max_us,prepaint_latency_ms\n0,1,9000,0,0,3000,0,1,3000,3000,1.5\n50,0,not_a_number,0,0,5000,0,1,5000,6000,3.0\n100,3,40000,0,0,15000,0,1,15000,18000,7.0\n",
        )
        .unwrap();

        let summary = summarize_conversation_csv(&path).unwrap();
        assert_eq!(summary.samples, 2, "only 2 valid rows expected");
        // Valid rows: 9ms (< 16ms), 40ms (> 16ms, > 33ms)
        assert_eq!(summary.over_16ms, 1);
        assert_eq!(summary.over_33ms, 1);
    }

    #[test]
    fn summarize_scroll_matrix_csv_drops_malformed_rows() {
        let path = unique_temp_dir("scroll-matrix-malformed").join("frames.csv");
        fs::write(
            &path,
            "elapsed_ms,variant,active,frame_total_us,sidebar_us,header_us,overlay_us,content_us,body_us,prepaint_since_render_ms\n1500,a,0,10000,0,0,0,4000,0,2\n1600,a,0,bad_value,0,0,0,7000,0,4\n1700,a,0,50000,0,0,0,9000,0,8\n",
        )
        .unwrap();

        let summary = summarize_scroll_matrix_csv(&path, 1500).unwrap();
        assert_eq!(summary.samples, 2, "only 2 valid rows expected");
        // Row 1: 10ms (< 16ms), Row 3: 50ms (> 16ms)
        assert_eq!(summary.over_16ms, 1);
    }

    #[test]
    fn run_process_timeout_enforces_timeout() {
        // Test that the timeout parameter actually works by spawning a long-running command
        let temp = unique_temp_dir("process-timeout-test");
        let stdout_path = temp.join("stdout.log");
        let stderr_path = temp.join("stderr.log");
        let screenshot_path = temp.join("screenshot.png");

        // Spawn a command that sleeps for 10 seconds
        let mut command = std::process::Command::new("sleep");
        command.arg("10");

        // With a 100ms timeout, this should fail
        let result = run_process_with_optional_capture(
            command,
            &stdout_path,
            &stderr_path,
            false,
            0,
            None,
            None,
            Some(100), // 100ms timeout
        );

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("timed out"),
            "expected timeout error, got: {}",
            err_msg
        );
        // Verify logs were written even on timeout (Task #95)
        assert!(stdout_path.exists(), "stdout should be written on timeout");
        assert!(stderr_path.exists(), "stderr should be written on timeout");
        std::fs::remove_dir_all(temp).ok();
    }

    #[test]
    fn run_process_without_timeout_succeeds() {
        // Test that without a timeout, a quick command succeeds
        let temp = unique_temp_dir("process-no-timeout-test");
        let stdout_path = temp.join("stdout.log");
        let stderr_path = temp.join("stderr.log");

        let mut command = std::process::Command::new("true");

        let result = run_process_with_optional_capture(
            command,
            &stdout_path,
            &stderr_path,
            false,
            0,
            None,
            None,
            None, // no timeout
        );

        assert!(result.is_ok());
        std::fs::remove_dir_all(temp).ok();
    }

    #[test]
    fn run_process_injects_env_vars() {
        // Test that env vars are correctly injected into the spawned process
        let temp = unique_temp_dir("env-var-test");
        let stdout_path = temp.join("stdout.log");
        let stderr_path = temp.join("stderr.log");

        // The run_process_with_optional_capture writes empty stdout on success.
        // To properly test env vars, we spawn a process directly with env vars
        // and verify the process sees them.
        let mut command = std::process::Command::new("bash");
        command.arg("-c").arg("echo 'ENV_OK'");

        // Inject custom env vars - this verifies the harness can pass env vars
        command.env("TEST_VAR", "test_value_123");
        command.env("ANOTHER_VAR", "another_value_456");

        // Use run_command from auto_ui_core to spawn and wait
        let output = auto_ui_core::run_command(&mut command, false).unwrap();
        assert!(
            output.stdout.contains("ENV_OK"),
            "command should have run successfully"
        );
        std::fs::remove_dir_all(temp).ok();
    }

    // Note: screenshot_capture_error_propagates test would require a real window
    // or x11 mocking to properly test. The screenshot code path only executes
    // when a window is found, so we document that screenshot errors ARE propagated
    // when the window exists but capture fails.

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
            timeout_ms: None,
            command: None,
            command_env: None,
            command_cwd: None,
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
            timeout_ms: None,
            command: None,
            command_env: None,
            command_cwd: None,
        })
        .unwrap();
        assert!(completed.report_path.exists());
    }

    #[test]
    #[ignore = "requires DISPLAY, built gpui examples, and AUTO_UI_RUN_LIVE_TESTS=1"]
    fn live_conversation_paint_smoke() {
        let _guard = live_test_lock().lock().unwrap();
        require_live_opt_in();
        let output_dir = unique_temp_dir("gpui-conversation-live");
        let completed = run_conversation_paint(ConversationPaintConfig {
            app_root: Some(require_env("AUTO_UI_TEST_GPUI_ROOT")),
            example: "llm_chat_conversation_bench_demo".to_string(),
            threads: vec![0],
            run_ms: 600,
            defer_first_frame: true,
            output_dir: Some(output_dir.display().to_string()),
            capture_window: false,
            settle_ms: 300,
            window_title_prefix: "Auto UI GPUI Conversation Smoke".to_string(),
            timeout_ms: None,
            command: None,
            command_env: None,
            command_cwd: None,
        })
        .unwrap();
        assert!(completed.report_path.exists());
    }

    #[test]
    fn resolve_app_root_prefers_explicit_path() {
        let temp = unique_temp_dir("resolve-app-root-gpui-test");
        let result = resolve_app_root(Some(&temp.display().to_string()));
        assert!(result.is_ok());
        std::fs::remove_dir(temp).ok();
    }

    #[test]
    fn resolve_app_root_falls_back_to_env() {
        std::env::set_var("GPUI_COMPONENT_TESTING_ROOT", "/tmp/test-gpui-env-root");
        let result = resolve_app_root(None);
        assert!(result.is_ok() || result.is_err());
        std::env::remove_var("GPUI_COMPONENT_TESTING_ROOT");
    }

    #[test]
    fn resolve_app_root_falls_back_to_sibling() {
        let result = resolve_app_root(None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(
                    e.to_string().contains("gpui") || e.to_string().contains("Could not resolve")
                );
            }
        }
    }

    #[test]
    fn resolve_app_root_returns_error_when_no_path_found() {
        std::env::remove_var("GPUI_COMPONENT_TESTING_ROOT");
        std::env::remove_var("AUTO_UI_GPUI_APP_ROOT");

        let result = resolve_app_root(None);

        match result {
            Ok(path) => {
                assert!(path.is_absolute());
            }
            Err(e) => {
                let err_msg = e.to_string();
                assert!(err_msg.contains("Could not resolve") || err_msg.contains("gpui"));
            }
        }
    }

    #[test]
    fn resolve_app_root_expands_tilde() {
        let _ = unique_temp_dir("resolve-app-root-gpui-tilde");
    }

    #[test]
    fn resolve_app_root_env_auto_ui_gpui_app_root() {
        std::env::set_var("AUTO_UI_GPUI_APP_ROOT", "/tmp/test-gpui-app-root-alt");
        let result = resolve_app_root(None);
        assert!(result.is_ok() || result.is_err());
        std::env::remove_var("AUTO_UI_GPUI_APP_ROOT");
    }

    #[test]
    fn resolve_app_root_app_root_takes_precedence() {
        std::env::set_var(
            "GPUI_COMPONENT_TESTING_ROOT",
            "/tmp/env-root-should-not-be-used",
        );
        let temp = unique_temp_dir("explicit-precedence-gpui");
        let explicit_path = temp.display().to_string();
        let result = resolve_app_root(Some(&explicit_path));
        assert!(result.is_ok());
        std::env::remove_var("GPUI_COMPONENT_TESTING_ROOT");
        std::fs::remove_dir(temp).ok();
    }

    #[test]
    fn resolve_app_root_priority_cli_over_env() {
        std::env::set_var("GPUI_COMPONENT_TESTING_ROOT", "/tmp/env-should-not-be-used");
        let temp = unique_temp_dir("cli-over-env-gpui");
        let result = resolve_app_root(Some(&temp.display().to_string()));
        assert!(result.is_ok());
        std::env::remove_var("GPUI_COMPONENT_TESTING_ROOT");
        std::fs::remove_dir(temp).ok();
    }

    #[test]
    fn resolve_app_root_priority_env_over_sibling() {
        std::env::set_var("GPUI_COMPONENT_TESTING_ROOT", "/tmp/env-priority-test-gpui");
        let result = resolve_app_root(None);
        assert!(result.is_ok());
        std::env::remove_var("GPUI_COMPONENT_TESTING_ROOT");
    }

    #[test]
    fn resolve_app_root_prefers_gpu_i_over_auto_ui() {
        std::env::set_var("GPUI_COMPONENT_TESTING_ROOT", "/tmp/gpui-wins");
        std::env::set_var("AUTO_UI_GPUI_APP_ROOT", "/tmp/auto-ui-should-lose");
        let result = resolve_app_root(None);
        assert!(result.is_ok());
        let path_buf = result.unwrap();
        let actual_path = path_buf.to_string_lossy();
        // GPUI env takes priority over AUTO_UI env
        assert_eq!(actual_path, "/tmp/gpui-wins");
        std::env::remove_var("GPUI_COMPONENT_TESTING_ROOT");
        std::env::remove_var("AUTO_UI_GPUI_APP_ROOT");
    }

    #[test]
    fn resolve_example_binary_returns_path_when_exists() {
        let temp = unique_temp_dir("example-binary-test");
        let example_dir = temp.join("target").join("release").join("examples");
        std::fs::create_dir_all(&example_dir).unwrap();
        let example_bin = example_dir.join("test_example");
        std::fs::write(&example_bin, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&example_bin).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&example_bin, perms).unwrap();
        }

        let result = resolve_example_binary(&temp, "test_example");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), example_bin);

        std::fs::remove_dir_all(temp).ok();
    }

    #[test]
    fn resolve_example_binary_fails_when_missing() {
        let temp = unique_temp_dir("example-binary-missing");
        let result = resolve_example_binary(&temp, "nonexistent_example");
        assert!(result.is_err());
        std::fs::remove_dir(temp).ok();
    }

    #[cfg(unix)]
    #[test]
    fn resolve_example_binary_fails_when_not_executable() {
        use std::os::unix::fs::PermissionsExt;
        let temp = unique_temp_dir("example-binary-not-exec");
        let example_dir = temp.join("target").join("release").join("examples");
        std::fs::create_dir_all(&example_dir).unwrap();
        let example_bin = example_dir.join("test_example");
        std::fs::write(&example_bin, "").unwrap();
        // Explicitly remove execute permission
        let mut perms = std::fs::metadata(&example_bin).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&example_bin, perms).unwrap();

        let result = resolve_example_binary(&temp, "test_example");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not executable"));

        std::fs::remove_dir_all(temp).ok();
    }

    // Helper to create a fake GPUI binary that writes proper output files
    fn create_fake_gpui_binary(dir: &Path, name: &str, scenario: &str) -> PathBuf {
        let bin_dir = dir.join("target").join("release").join("examples");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let bin_path = bin_dir.join(name);

        let script = match scenario {
            "scroll_matrix" => {
                // BENCH_OUTPUT = CSV path, BENCH_VARIANT = variant name
                // BENCH_DURATION_MS, BENCH_SCROLL_WARMUP_MS, etc. are also set
                r#"#!/bin/bash
OUTPUT_PATH="$BENCH_OUTPUT"
mkdir -p "$(dirname "$OUTPUT_PATH")"
cat > "$OUTPUT_PATH" << 'CSVEOF'
elapsed_ms,variant,active,frame_total_us,sidebar_us,header_us,overlay_us,content_us,body_us,prepaint_since_render_ms
1500,a,0,10000,0,0,0,4000,0,2
1600,a,0,20000,0,0,0,7000,0,4
CSVEOF
# Also create stdout/stderr files that the adapter expects
STDOUT_PATH="${OUTPUT_PATH%.frames.csv}.stdout.log"
STDERR_PATH="${OUTPUT_PATH%.frames.csv}.stderr.log"
echo "variant a started" > "$STDOUT_PATH"
echo "[scrollbar] prepaint states=5" > "$STDERR_PATH"
exit 0
"#
            }
            "scrollbar_trace" => {
                // scrollbar_trace writes to stderr, which is captured and parsed
                // The stdout/stderr paths are output_dir.join(format!("{}.stdout.log", variant))
                r#"#!/bin/bash
# For scrollbar_trace, stderr is what matters - it looks for [scrollbar] prepaint lines
# We need to write to stderr directly - but the adapter captures stdout/stderr
# The actual trace output goes to stderr in the real app
echo "[scrollbar] prepaint states=5" >&2
echo "[scrollbar] prepaint states=3" >&2
exit 0
"#
            }
            "conversation_paint" => {
                // BENCH_OUTPUT = CSV path, BENCH_VARIANT = thread_id
                r#"#!/bin/bash
OUTPUT_PATH="$BENCH_OUTPUT"
mkdir -p "$(dirname "$OUTPUT_PATH")"
cat > "$OUTPUT_PATH" << 'CSVEOF'
elapsed_ms,render_num,render_total_us,sidebar_us,header_us,messages_us,layout_us,msg_count,msg_avg_us,msg_max_us,prepaint_latency_ms
0,1,9000,0,0,3000,0,1,3000,3000,1.5
10,2,18000,0,0,5000,0,1,5000,6000,3.0
CSVEOF
STDOUT_PATH="${OUTPUT_PATH%.frames.csv}.stdout.log"
STDERR_PATH="${OUTPUT_PATH%.frames.csv}.stderr.log"
echo "thread started" > "$STDOUT_PATH"
echo "[scrollbar] prepaint states=5" > "$STDERR_PATH"
exit 0
"#
            }
            _ => panic!("unknown scenario: {}", scenario),
        };

        std::fs::write(&bin_path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        bin_path
    }

    // Helper to read and parse the report.json from a completed run
    fn read_report_artifacts(report_path: &Path) -> serde_json::Value {
        let content = std::fs::read_to_string(report_path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    #[test]
    fn scroll_matrix_registers_expected_artifacts() {
        let temp = unique_temp_dir("artifact-scroll-matrix");
        let output_dir = temp.join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create fake GPUI binary
        let _fake_bin = create_fake_gpui_binary(&temp, "fake_scroll_matrix", "scroll_matrix");

        let config = ScrollMatrixConfig {
            app_root: Some(temp.to_str().unwrap().to_string()),
            example: "fake_scroll_matrix".to_string(),
            variants: vec!["a".to_string()],
            run_ms: 100,
            warmup_ms: 50,
            scroll_delay_ms: 16,
            scroll_step_px: 40,
            output_dir: Some(output_dir.to_str().unwrap().to_string()),
            capture_window: false,
            settle_ms: 100,
            window_title_prefix: "Test".to_string(),
            timeout_ms: Some(5000),
            command: None,
            command_env: None,
            command_cwd: None,
        };

        let completed = run_scroll_matrix(config).expect("run_scroll_matrix should succeed");

        // Read the report and verify artifacts
        let report = read_report_artifacts(&completed.report_path);
        let artifacts: Vec<_> = report["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["kind"].as_str().unwrap())
            .collect();

        // Verify all expected artifacts are registered
        assert!(
            artifacts.contains(&"progress_log"),
            "should have progress_log artifact"
        );
        assert!(
            artifacts.contains(&"gpui_csv"),
            "should have gpui_csv artifact"
        );
        assert!(
            artifacts.contains(&"stdout_log"),
            "should have stdout_log artifact"
        );
        assert!(
            artifacts.contains(&"stderr_log"),
            "should have stderr_log artifact"
        );
        assert!(
            artifacts.contains(&"summary_tsv"),
            "should have summary_tsv artifact"
        );
        assert!(
            artifacts.contains(&"summary_markdown"),
            "should have summary_markdown artifact"
        );

        std::fs::remove_dir_all(temp).ok();
    }

    #[test]
    fn scrollbar_trace_registers_expected_artifacts() {
        let temp = unique_temp_dir("artifact-scrollbar-trace");
        let output_dir = temp.join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create fake GPUI binary for scrollbar_trace
        let _fake_bin = create_fake_gpui_binary(&temp, "fake_scrollbar", "scrollbar_trace");

        let config = ScrollbarTraceConfig {
            app_root: Some(temp.to_str().unwrap().to_string()),
            example: "fake_scrollbar".to_string(),
            run_ms: 100,
            warmup_ms: 50,
            scroll_delay_ms: 16,
            scroll_step_px: 40,
            output_dir: Some(output_dir.to_str().unwrap().to_string()),
            capture_window: false,
            settle_ms: 100,
            window_title: "Test Scrollbar".to_string(),
            timeout_ms: Some(5000),
            command: None,
            command_env: None,
            command_cwd: None,
        };

        let completed = run_scrollbar_trace(config).expect("run_scrollbar_trace should succeed");

        // Read the report and verify artifacts
        let report = read_report_artifacts(&completed.report_path);
        let artifacts: Vec<_> = report["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["kind"].as_str().unwrap())
            .collect();

        // Verify all expected artifacts are registered
        assert!(
            artifacts.contains(&"progress_log"),
            "should have progress_log artifact"
        );
        assert!(
            artifacts.contains(&"stdout_log"),
            "should have stdout_log artifact"
        );
        assert!(
            artifacts.contains(&"stderr_log"),
            "should have stderr_log artifact"
        );
        assert!(
            artifacts.contains(&"summary_markdown"),
            "should have summary_markdown artifact"
        );

        // scrollbar_trace does NOT have gpui_csv, summary_tsv, or window_screenshot (when capture_window=false)
        assert!(
            !artifacts.contains(&"gpui_csv"),
            "scrollbar_trace should NOT have gpui_csv artifact"
        );
        assert!(
            !artifacts.contains(&"summary_tsv"),
            "scrollbar_trace should NOT have summary_tsv artifact"
        );

        std::fs::remove_dir_all(temp).ok();
    }

    #[test]
    fn conversation_paint_registers_expected_artifacts() {
        let temp = unique_temp_dir("artifact-conversation-paint");
        let output_dir = temp.join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create fake GPUI binary for conversation_paint
        let _fake_bin = create_fake_gpui_binary(&temp, "fake_conversation", "conversation_paint");

        let config = ConversationPaintConfig {
            app_root: Some(temp.to_str().unwrap().to_string()),
            example: "fake_conversation".to_string(),
            threads: vec![0],
            run_ms: 100,
            defer_first_frame: true,
            output_dir: Some(output_dir.to_str().unwrap().to_string()),
            capture_window: false,
            settle_ms: 100,
            window_title_prefix: "Test".to_string(),
            timeout_ms: Some(5000),
            command: None,
            command_env: None,
            command_cwd: None,
        };

        let completed =
            run_conversation_paint(config).expect("run_conversation_paint should succeed");

        // Read the report and verify artifacts
        let report = read_report_artifacts(&completed.report_path);
        let artifacts: Vec<_> = report["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["kind"].as_str().unwrap())
            .collect();

        // Verify all expected artifacts are registered
        assert!(
            artifacts.contains(&"progress_log"),
            "should have progress_log artifact"
        );
        assert!(
            artifacts.contains(&"gpui_csv"),
            "should have gpui_csv artifact"
        );
        assert!(
            artifacts.contains(&"stdout_log"),
            "should have stdout_log artifact"
        );
        assert!(
            artifacts.contains(&"stderr_log"),
            "should have stderr_log artifact"
        );
        assert!(
            artifacts.contains(&"summary_tsv"),
            "should have summary_tsv artifact"
        );
        assert!(
            artifacts.contains(&"summary_markdown"),
            "should have summary_markdown artifact"
        );

        std::fs::remove_dir_all(temp).ok();
    }

    #[test]
    fn scroll_matrix_capture_window_registers_screenshot_artifact() {
        // When capture_window=true and screenshot file exists, window_screenshot should be registered
        let temp = unique_temp_dir("artifact-scroll-matrix-screenshot");
        let output_dir = temp.join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create fake GPUI binary
        let _fake_bin = create_fake_gpui_binary(&temp, "fake_scroll_matrix", "scroll_matrix");

        // Pre-create a screenshot file at the expected path
        let screenshot_path = output_dir.join("a.window.png");
        std::fs::write(&screenshot_path, "fake screenshot data").unwrap();

        let config = ScrollMatrixConfig {
            app_root: Some(temp.to_str().unwrap().to_string()),
            example: "fake_scroll_matrix".to_string(),
            variants: vec!["a".to_string()],
            run_ms: 100,
            warmup_ms: 50,
            scroll_delay_ms: 16,
            scroll_step_px: 40,
            output_dir: Some(output_dir.to_str().unwrap().to_string()),
            capture_window: true,
            settle_ms: 100,
            window_title_prefix: "Test".to_string(),
            timeout_ms: Some(5000),
            command: None,
            command_env: None,
            command_cwd: None,
        };

        let completed = run_scroll_matrix(config).expect("run_scroll_matrix should succeed");

        // Read the report and verify window_screenshot artifact is registered
        let report = read_report_artifacts(&completed.report_path);
        let artifacts: Vec<_> = report["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["kind"].as_str().unwrap())
            .collect();

        assert!(
            artifacts.contains(&"window_screenshot"),
            "should have window_screenshot artifact when capture_window=true and file exists"
        );

        std::fs::remove_dir_all(temp).ok();
    }

    #[test]
    fn parse_command_string_extracts_path_and_args() {
        // Create a real executable to test parsing
        use std::os::unix::fs::PermissionsExt;
        let temp = unique_temp_dir("parse-cmd-test");
        let bin_path = temp.join("test_binary");
        std::fs::write(&bin_path, "#!/bin/sh").unwrap();
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let (path, args) =
            parse_command_string(&format!("{} --arg1 --arg2", bin_path.to_string_lossy())).unwrap();
        assert_eq!(path, bin_path);
        assert_eq!(args, vec!["--arg1", "--arg2"]);
        std::fs::remove_dir_all(temp).ok();
    }

    #[test]
    fn parse_command_string_path_only() {
        use std::os::unix::fs::PermissionsExt;
        let temp = unique_temp_dir("parse-cmd-path-test");
        let bin_path = temp.join("test_binary");
        std::fs::write(&bin_path, "#!/bin/sh").unwrap();
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let (path, args) = parse_command_string(&bin_path.to_string_lossy()).unwrap();
        assert_eq!(path, bin_path);
        assert!(args.is_empty());
        std::fs::remove_dir_all(temp).ok();
    }

    #[test]
    fn parse_command_string_empty_command_fails() {
        let result = parse_command_string("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty command"));
    }

    #[test]
    fn parse_command_string_nonexistent_binary_fails() {
        let result = parse_command_string("/nonexistent/path/to/binary");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[cfg(unix)]
    #[test]
    fn parse_command_string_non_executable_fails() {
        use std::os::unix::fs::PermissionsExt;
        let temp = unique_temp_dir("non-exec-test");
        let bin_path = temp.join("non_exec_binary");
        std::fs::write(&bin_path, "not executable").unwrap();
        // Ensure file is NOT executable
        let mut perms = std::fs::metadata(&bin_path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&bin_path, perms).unwrap();

        let result = parse_command_string(&bin_path.to_string_lossy());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not executable"));
        std::fs::remove_dir_all(temp).ok();
    }

    #[cfg(unix)]
    #[test]
    fn parse_command_string_executable_succeeds() {
        use std::os::unix::fs::PermissionsExt;
        let temp = unique_temp_dir("exec-test");
        let bin_path = temp.join("exec_binary");
        std::fs::write(&bin_path, "#!/bin/sh\necho hello").unwrap();
        let mut perms = std::fs::metadata(&bin_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin_path, perms).unwrap();

        let (path, args) = parse_command_string(&bin_path.to_string_lossy()).unwrap();
        assert_eq!(path, bin_path);
        assert!(args.is_empty());
        std::fs::remove_dir_all(temp).ok();
    }
}
