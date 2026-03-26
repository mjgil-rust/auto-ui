use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use auto_ui_artifacts::{write_report, Report};
use auto_ui_core::{
    build_output_dir, expand_path, home_dir, log_line, normalize_name, parse_widths, repo_root,
    request_background_launch, run_command, CompletedRun, TraceFields,
};
use auto_ui_driver_x11 as x11;
use clap::ValueEnum;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const TARGET_ID: &str = "rust_chatbot";
pub const SCENARIOS: &[&str] = &["debug", "header_debug"];
const DEFAULT_SESSION_NAMES: &[&str] = &[
    "pl-update",
    "pl-enhance",
    "pl-24",
    "pl-assess",
    "da-scrape-result-submission",
    "pl-graph-problem",
    "pl-enhancements-2",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
    Gemini,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
            Provider::Gemini => "gemini",
        }
    }

    pub fn title_base(self) -> &'static str {
        match self {
            Provider::Claude => "Claude Rust Chatbot",
            Provider::Codex => "Codex Rust Chatbot",
            Provider::Gemini => "Gemini Rust Chatbot",
        }
    }

    pub fn data_dir_name(self) -> &'static str {
        match self {
            Provider::Claude => ".claude-desktop",
            Provider::Codex => ".codex-desktop",
            Provider::Gemini => ".gemini-desktop",
        }
    }

    pub fn provider_session_field(self) -> &'static str {
        match self {
            Provider::Claude => "claude_session_id",
            Provider::Codex => "codex_session_id",
            Provider::Gemini => "gemini_session_id",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SessionEntry {
    pub session_id: String,
    pub name: String,
    pub updated_at: String,
    pub message_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebugConfig {
    pub app_root: Option<String>,
    pub provider: Provider,
    pub instance: Option<u32>,
    pub widths: String,
    pub height: u32,
    pub max_sessions: usize,
    pub session_id: Option<String>,
    pub include_hidden: bool,
    pub launch_if_missing: bool,
    pub window_timeout: f64,
    pub trace_timeout: f64,
    pub settle: f64,
    pub output_dir: Option<String>,
    pub keep_front: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            app_root: None,
            provider: Provider::Codex,
            instance: None,
            widths: "520,900,1000".to_string(),
            height: 900,
            max_sessions: 8,
            session_id: None,
            include_hidden: false,
            launch_if_missing: true,
            window_timeout: 15.0,
            trace_timeout: 5.0,
            settle: 0.7,
            output_dir: None,
            keep_front: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeaderDebugConfig {
    pub app_root: Option<String>,
    pub provider: Provider,
    pub instance: Option<u32>,
    pub session_id: Option<String>,
    pub session_name: Option<String>,
    pub include_hidden: bool,
    pub widths: String,
    pub height: u32,
    pub header_height: i32,
    pub window_timeout: f64,
    pub trace_timeout: f64,
    pub settle: f64,
    pub output_dir: Option<String>,
    pub keep_front: bool,
}

impl Default for HeaderDebugConfig {
    fn default() -> Self {
        Self {
            app_root: None,
            provider: Provider::Codex,
            instance: None,
            session_id: None,
            session_name: None,
            include_hidden: false,
            widths: "520,900,1000".to_string(),
            height: 900,
            header_height: 140,
            window_timeout: 15.0,
            trace_timeout: 8.0,
            settle: 0.8,
            output_dir: None,
            keep_front: false,
        }
    }
}

#[derive(Clone, Debug)]
struct SessionDetails {
    session_id: String,
    name: String,
    updated_at: String,
    message_count: usize,
    provider_session_id: Option<String>,
    launched_from: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CropBox {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

pub fn scenario_names() -> &'static [&'static str] {
    SCENARIOS
}

pub fn validate_named_scenario(scenario: &str, value: &Value) -> Result<()> {
    match normalize_name(scenario).as_str() {
        "debug" => {
            debug_config_from_scenario(value.clone(), None)?;
            Ok(())
        }
        "header_debug" => {
            header_config_from_scenario(value.clone(), None)?;
            Ok(())
        }
        other => bail!("Unsupported rust-chatbot scenario {other:?}."),
    }
}

pub fn run_named_scenario(
    scenario: &str,
    value: Value,
    output_override: Option<String>,
) -> Result<CompletedRun> {
    match normalize_name(scenario).as_str() {
        "debug" => {
            let config = debug_config_from_scenario(value, output_override)?;
            run_debug(config)
        }
        "header_debug" => {
            let config = header_config_from_scenario(value, output_override)?;
            run_header_debug(config)
        }
        other => bail!("Unsupported rust-chatbot scenario {other:?}."),
    }
}

pub fn run_debug(config: DebugConfig) -> Result<CompletedRun> {
    auto_ui_core::ensure_display("Rust Chatbot")?;

    let app_root = resolve_app_root(config.app_root.as_deref())?;
    require_release_binaries(&app_root, &["chatbot-ctl", "rust-chatbot"])?;
    let desktop_window_id = x11::get_active_window_id()?;

    let widths = parse_widths(&config.widths)?;
    let sessions = if let Some(session_id) = config.session_id.as_deref() {
        vec![load_session_by_id(config.provider, session_id)?]
    } else {
        load_sessions(
            config.provider,
            config.max_sessions,
            config.include_hidden,
            Some(DEFAULT_SESSION_NAMES),
        )?
    };
    if sessions.is_empty() {
        bail!("No visible sessions with messages were found for that provider.");
    }

    let title = provider_title(config.provider, &app_root);
    let output_dir = build_output_dir(config.output_dir.as_deref(), "auto-ui-debug")?;
    let progress_path = output_dir.join("progress.log");
    let log_path = newest_trace_log()?;
    let mut log_offset = fs::metadata(&log_path)?.len();

    log_line(
        format!("app_root={}", app_root.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("output_dir={}", output_dir.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("trace_log={}", log_path.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!(
            "desktop_window_id={}",
            desktop_window_id.as_deref().unwrap_or("<none>")
        ),
        Some(&progress_path),
    )?;

    let per_session_launch_mode = config.launch_if_missing && config.session_id.is_none();
    let mut window_id: Option<String> = None;
    let mut interaction_window_id: Option<String> = None;
    let mut launched_debug_pid: Option<i32> = None;
    let mut restore_window_geometry: Option<(String, x11::WindowGeometry)> = None;

    if !per_session_launch_mode {
        if config.launch_if_missing {
            let existing_pids = list_chatbot_pids(&app_root)?;
            let existing_window_ids: HashSet<_> =
                x11::find_window_ids(&title)?.into_iter().collect();
            let existing_interaction_window_ids: HashSet<_> =
                x11::find_interaction_window_ids(&title)?
                    .into_iter()
                    .collect();
            log_line(
                format!("launching fresh {title} with RUST_CHATBOT_AUTO_UI_DEBUG=1"),
                Some(&progress_path),
            )?;
            launch_window(
                &app_root,
                config.provider,
                config.instance,
                config.session_id.as_deref(),
            )?;
            let launched_pid =
                wait_for_new_pid(&app_root, &existing_pids, seconds(config.window_timeout))?;
            if let Some(launched_pid) = launched_pid {
                launched_debug_pid = Some(launched_pid);
                log_line(
                    format!("detected launched_pid={launched_pid}"),
                    Some(&progress_path),
                )?;
                window_id =
                    x11::find_window_id_for_pid(launched_pid, seconds(config.window_timeout))?;
                interaction_window_id = x11::find_interaction_window_id_for_pid(
                    launched_pid,
                    seconds(config.window_timeout),
                )?;
            }

            if window_id.is_none() {
                window_id = x11::wait_for_new_window_id(
                    &title,
                    &existing_window_ids,
                    seconds(config.window_timeout),
                )?;
            }
            let Some(existing_window_id) = window_id.clone() else {
                bail!("Could not find a new window matching {title:?} after launch.");
            };
            if interaction_window_id.is_none() {
                interaction_window_id = x11::wait_for_new_interaction_window_id(
                    &title,
                    &existing_interaction_window_ids,
                    seconds(config.window_timeout),
                )?;
            }
            if !config.keep_front {
                x11::background_window(&existing_window_id, desktop_window_id.as_deref())?;
            }
        } else {
            window_id = x11::find_window_id(&title, seconds(config.window_timeout))?;
            if window_id.is_none() {
                bail!(
                    "No window matching {title:?} was found. Re-run without --no-launch to let the tool start one."
                );
            }
            interaction_window_id =
                x11::find_interaction_window_id(&title, seconds(config.window_timeout))?;
        }

        if interaction_window_id.is_none() {
            interaction_window_id = window_id.clone();
        }
        if !config.launch_if_missing {
            if let Some(existing_window_id) = window_id.as_deref() {
                let geometry = x11::get_window_geometry(existing_window_id)?;
                log_line(
                    format!(
                        "captured original geometry window_id={existing_window_id} x={} y={} width={} height={}",
                        geometry.x, geometry.y, geometry.width, geometry.height
                    ),
                    Some(&progress_path),
                )?;
                restore_window_geometry = Some((existing_window_id.to_string(), geometry));
            }
        }

        log_line(
            format!(
                "using window_id={}",
                window_id.as_deref().unwrap_or("<none>")
            ),
            Some(&progress_path),
        )?;
        log_line(
            format!(
                "using interaction_window_id={}",
                interaction_window_id.as_deref().unwrap_or("<none>")
            ),
            Some(&progress_path),
        )?;
    }

    let mut report_sessions = Vec::new();
    let mut report = Report::new(TARGET_ID, "debug", "hybrid", &app_root);
    report.add_artifact(
        "progress_log",
        progress_path.display().to_string(),
        Some("live progress log".to_string()),
        Value::Null,
    );
    report.add_artifact(
        "trace_log",
        log_path.display().to_string(),
        Some("rust-chatbot trace log".to_string()),
        Value::Null,
    );
    let result = (|| -> Result<CompletedRun> {
        for width in &widths {
            let mut session_entries = Vec::new();

            for session in &sessions {
                match run_width_session(
                    &config,
                    &app_root,
                    &title,
                    &log_path,
                    &progress_path,
                    desktop_window_id.as_deref(),
                    per_session_launch_mode,
                    &window_id,
                    interaction_window_id.as_deref(),
                    *width,
                    session,
                    &mut log_offset,
                    &output_dir,
                    &mut report,
                ) {
                    Ok(entry) => session_entries.push(entry),
                    Err(err) => {
                        log_line(
                            format!("trace timeout for session={}: {err}", session.name),
                            Some(&progress_path),
                        )?;
                        session_entries.push(json!({
                            "session_id": session.session_id,
                            "session_name": session.name,
                            "error": format!(
                                "{err}. If you attached to an already-open window, it was probably not started with RUST_CHATBOT_AUTO_UI_DEBUG=1."
                            ),
                        }));
                    }
                }
            }

            report_sessions.push(json!({
                "requested_width": width,
                "window_geometry": Value::Null,
                "sessions": session_entries,
            }));
        }

        report.set_details(json!({
            "provider": config.provider.as_str(),
            "instance": config.instance,
            "window_id": window_id,
            "title_substring": title,
            "widths": widths,
            "height": config.height,
            "sessions": report_sessions,
            "output_dir": output_dir,
            "trace_log": log_path,
        }));
        report.finish_ok();
        let report_path = write_report(&output_dir, &report)?;

        println!("wrote {}", report_path.display());
        if let Some(width_entries) = report.details.get("sessions").and_then(Value::as_array) {
            for width_entry in width_entries {
                println!(
                    "\nwidth {}:",
                    width_entry
                        .get("requested_width")
                        .and_then(Value::as_u64)
                        .unwrap_or_default()
                );
                if let Some(entries) = width_entry.get("sessions").and_then(Value::as_array) {
                    for entry in entries {
                        if let Some(error) = entry.get("error").and_then(Value::as_str) {
                            println!(
                                "  {}: ERROR {}",
                                entry
                                    .get("session_name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("<unknown>"),
                                error
                            );
                            continue;
                        }
                        let trace = entry.get("trace").unwrap_or(&Value::Null);
                        let overflow = trace
                            .get("max_rendered_overflow")
                            .and_then(Value::as_str)
                            .unwrap_or("n/a");
                        let text_ok = if entry
                            .get("text_visible_heuristic")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                        {
                            "yes"
                        } else {
                            "no"
                        };
                        let area_width = trace
                            .get("message_area_available_width")
                            .and_then(Value::as_str)
                            .unwrap_or("n/a");
                        let code_block_count = entry
                            .get("code_block_traces")
                            .and_then(Value::as_array)
                            .map(|items| items.len())
                            .unwrap_or(0);
                        println!(
                            "  {}: area_width={} overflow={} code_blocks={} text_visible={}",
                            entry
                                .get("session_name")
                                .and_then(Value::as_str)
                                .unwrap_or("<unknown>"),
                            area_width,
                            overflow,
                            code_block_count,
                            text_ok
                        );
                    }
                }
            }
        }

        println!("\nlogs:");
        println!("  progress: {}", progress_path.display());
        println!("  trace: {}", log_path.display());
        println!("  report: {}", report_path.display());

        Ok(CompletedRun {
            output_dir,
            report_path,
        })
    })();

    if let Some((existing_window_id, geometry)) = restore_window_geometry {
        let restore_result = restore_reused_window(
            &existing_window_id,
            &geometry,
            config.keep_front,
            desktop_window_id.as_deref(),
            Some(&progress_path),
        );
        if result.is_ok() {
            restore_result?;
        } else if let Err(err) = restore_result {
            let _ = log_line(
                format!("warning: failed to restore window_id={existing_window_id}: {err:#}"),
                Some(&progress_path),
            );
        }
    }

    if let Some(launched_pid) = launched_debug_pid {
        let stop_result = stop_chatbot_pid(&app_root, launched_pid).and_then(|_| {
            wait_for_pid_exit(&app_root, launched_pid, seconds(config.window_timeout))
        });
        if result.is_ok() {
            stop_result?;
        } else if let Err(err) = stop_result {
            let _ = log_line(
                format!("warning: failed to stop launched_pid={launched_pid}: {err:#}"),
                Some(&progress_path),
            );
        }
    }

    result
}

pub fn run_header_debug(config: HeaderDebugConfig) -> Result<CompletedRun> {
    auto_ui_core::ensure_display("Rust Chatbot")?;

    let widths = parse_widths(&config.widths)?;
    if widths.is_empty() {
        bail!("At least one width is required.");
    }

    let app_root = resolve_app_root(config.app_root.as_deref())?;
    require_release_binaries(&app_root, &["chatbot-ctl", "rust-chatbot"])?;
    let desktop_window_id = x11::get_active_window_id()?;
    let session = resolve_session(
        config.provider,
        config.session_id.as_deref(),
        config.session_name.as_deref(),
        config.include_hidden,
    )?;
    let title = provider_title(config.provider, &app_root);
    let output_dir = build_output_dir(config.output_dir.as_deref(), "auto-ui-header-debug")?;
    let progress_path = output_dir.join("progress.log");
    let log_path = newest_trace_log()?;
    let startup_offset = fs::metadata(&log_path)?.len();

    log_line(
        format!("app_root={}", app_root.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("output_dir={}", output_dir.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!("trace_log={}", log_path.display()),
        Some(&progress_path),
    )?;
    log_line(
        format!(
            "desktop_window_id={}",
            desktop_window_id.as_deref().unwrap_or("<none>")
        ),
        Some(&progress_path),
    )?;
    log_line(
        format!("session_id={}", session.session_id),
        Some(&progress_path),
    )?;
    log_line(
        format!("session_name={}", session.name),
        Some(&progress_path),
    )?;
    log_line(
        format!(
            "provider_session_id={}",
            session.provider_session_id.as_deref().unwrap_or("<none>")
        ),
        Some(&progress_path),
    )?;
    log_line(
        format!(
            "launched_from={}",
            session.launched_from.as_deref().unwrap_or("<none>")
        ),
        Some(&progress_path),
    )?;

    let mut launched_pid = None;
    let mut report = Report::new(TARGET_ID, "header_debug", "hybrid", &app_root);
    report.add_artifact(
        "progress_log",
        progress_path.display().to_string(),
        Some("live progress log".to_string()),
        Value::Null,
    );
    report.add_artifact(
        "trace_log",
        log_path.display().to_string(),
        Some("rust-chatbot trace log".to_string()),
        Value::Null,
    );

    let result = (|| -> Result<CompletedRun> {
        let (new_pid, window_id) = launch_targeted_session_window(
            &app_root,
            config.provider,
            config.instance,
            &title,
            &session.session_id,
            config.keep_front,
            seconds(config.window_timeout),
            Some(&progress_path),
            desktop_window_id.as_deref(),
        )?;
        launched_pid = Some(new_pid);

        let (mut current_offset, startup_trace, startup_code_blocks) = wait_for_trace_bundle(
            &log_path,
            startup_offset,
            &session.session_id,
            seconds(config.trace_timeout.max(10.0)),
            seconds(0.5),
        )?;
        log_line(
            format!(
                "startup trace captured content_width={} first_rect_min_x={}",
                startup_trace
                    .get("content_width")
                    .map(String::as_str)
                    .unwrap_or("n/a"),
                startup_trace
                    .get("first_rect_min_x")
                    .map(String::as_str)
                    .unwrap_or("n/a")
            ),
            Some(&progress_path),
        )?;

        let mut report_widths = Vec::new();

        for width in widths {
            log_line(
                format!("resizing window to width={width} height={}", config.height),
                Some(&progress_path),
            )?;
            let geometry = if config.keep_front {
                x11::resize_window(&window_id, width, config.height)?
            } else {
                x11::prepare_window_for_capture(
                    &window_id,
                    width,
                    config.height,
                    desktop_window_id.as_deref(),
                )?
            };
            thread::sleep(seconds(config.settle));

            let (new_offset, trace, code_block_traces) = match wait_for_trace_bundle(
                &log_path,
                current_offset,
                &session.session_id,
                seconds(config.trace_timeout),
                seconds(0.5),
            ) {
                Ok(result) => result,
                Err(_) => {
                    log_line(
                        format!("resize produced no new ui trace for width={width}; reusing startup trace"),
                        Some(&progress_path),
                    )?;
                    (
                        current_offset,
                        startup_trace.clone(),
                        startup_code_blocks.clone(),
                    )
                }
            };
            current_offset = new_offset;

            let screenshot_path = output_dir.join(format!(
                "{}-w{}-{}-window.png",
                config.provider.as_str(),
                width,
                &session.session_id[..8]
            ));
            let top_strip_path = output_dir.join(format!(
                "{}-w{}-{}-top-strip.png",
                config.provider.as_str(),
                width,
                &session.session_id[..8]
            ));
            let focus_path = output_dir.join(format!(
                "{}-w{}-{}-header-focus.png",
                config.provider.as_str(),
                width,
                &session.session_id[..8]
            ));
            let focus_enhanced_path = output_dir.join(format!(
                "{}-w{}-{}-header-focus-enhanced.png",
                config.provider.as_str(),
                width,
                &session.session_id[..8]
            ));

            x11::capture_window_screenshot(&window_id, &screenshot_path)?;
            let (screenshot_width, screenshot_height) = x11::image_size(&screenshot_path)?;
            let top_strip_height = clamp(config.header_height, 1, screenshot_height);
            crop_image(
                &screenshot_path,
                &top_strip_path,
                0,
                0,
                screenshot_width,
                top_strip_height,
            )?;

            let focus_crop = approximate_header_focus_crop(
                &trace,
                &geometry,
                screenshot_width,
                screenshot_height,
                config.header_height,
            );
            crop_image(
                &screenshot_path,
                &focus_path,
                focus_crop.x,
                focus_crop.y,
                focus_crop.width,
                focus_crop.height,
            )?;
            enhance_image(&focus_path, &focus_enhanced_path)?;

            let top_strip_metric =
                x11::crop_metric(&screenshot_path, 0, 0, screenshot_width, top_strip_height)?;
            let focus_metric = x11::crop_metric(
                &screenshot_path,
                focus_crop.x,
                focus_crop.y,
                focus_crop.width,
                focus_crop.height,
            )?;

            report.add_artifact(
                "window_screenshot",
                screenshot_path.display().to_string(),
                Some(format!("header-debug width {width}")),
                json!({ "width": width }),
            );
            report.add_artifact(
                "top_strip",
                top_strip_path.display().to_string(),
                Some(format!("top strip width {width}")),
                json!({ "width": width, "height": top_strip_height }),
            );
            report.add_artifact(
                "header_focus",
                focus_path.display().to_string(),
                Some(format!("header focus width {width}")),
                json!({ "width": width }),
            );
            report.add_artifact(
                "header_focus_enhanced",
                focus_enhanced_path.display().to_string(),
                Some(format!("enhanced header focus width {width}")),
                json!({ "width": width }),
            );

            report_widths.push(json!({
                "requested_width": width,
                "window_geometry": geometry,
                "trace": trace,
                "code_block_traces": code_block_traces,
                "screenshot": screenshot_path,
                "top_strip": {
                    "path": top_strip_path,
                    "height": top_strip_height,
                    "metric": top_strip_metric,
                },
                "header_focus": {
                    "path": focus_path,
                    "enhanced_path": focus_enhanced_path,
                    "crop": focus_crop,
                    "metric": focus_metric,
                },
            }));

            log_line(
                format!(
                    "captured width={} focus_crop=({},{},{},{}) focus_stddev={:.4}",
                    width,
                    focus_crop.x,
                    focus_crop.y,
                    focus_crop.width,
                    focus_crop.height,
                    focus_metric.stddev
                ),
                Some(&progress_path),
            )?;
        }

        report.set_details(json!({
            "provider": config.provider.as_str(),
            "instance": config.instance,
            "title_substring": title,
            "window_id": window_id,
            "trace_log": log_path,
            "output_dir": output_dir,
            "session": {
                "session_id": session.session_id,
                "name": session.name,
                "updated_at": session.updated_at,
                "message_count": session.message_count,
                "provider_session_id": session.provider_session_id,
                "launched_from": session.launched_from,
            },
            "startup_trace": startup_trace,
            "startup_code_block_traces": startup_code_blocks,
            "widths": report_widths,
            "ocr_available": false,
        }));
        report.finish_ok();
        let report_path = write_report(&output_dir, &report)?;

        println!("wrote {}", report_path.display());
        println!("\nexpected header values:");
        println!("  session name: {}", session.name);
        println!(
            "  provider session id: {}",
            session.provider_session_id.as_deref().unwrap_or("<none>")
        );
        println!(
            "  launched_from: {}",
            session.launched_from.as_deref().unwrap_or("<none>")
        );
        println!("\nartifacts:");
        println!("  progress: {}", progress_path.display());
        println!("  report: {}", report_path.display());
        if let Some(entries) = report.details.get("widths").and_then(Value::as_array) {
            for entry in entries {
                println!(
                    "  width {}: window={} header={} enhanced={}",
                    entry
                        .get("requested_width")
                        .and_then(Value::as_u64)
                        .unwrap_or_default(),
                    entry
                        .get("screenshot")
                        .and_then(Value::as_str)
                        .unwrap_or("<none>"),
                    entry
                        .get("header_focus")
                        .and_then(|value| value.get("path"))
                        .and_then(Value::as_str)
                        .unwrap_or("<none>"),
                    entry
                        .get("header_focus")
                        .and_then(|value| value.get("enhanced_path"))
                        .and_then(Value::as_str)
                        .unwrap_or("<none>")
                );
            }
        }

        Ok(CompletedRun {
            output_dir,
            report_path,
        })
    })();

    if let Some(launched_pid) = launched_pid {
        stop_chatbot_pid(&app_root, launched_pid)?;
        wait_for_pid_exit(&app_root, launched_pid, seconds(config.window_timeout))?;
    }

    result
}

fn debug_config_from_scenario(
    value: Value,
    output_override: Option<String>,
) -> Result<DebugConfig> {
    let scenario: DebugScenarioFile = serde_json::from_value(value)?;
    let mut config = DebugConfig::default();
    if let Some(app) = scenario.app {
        config.app_root = app.root;
        config.provider = app.provider.unwrap_or(config.provider);
        config.instance = app.instance;
        config.session_id = app.session_id;
        config.include_hidden = app.include_hidden.unwrap_or(config.include_hidden);
        config.max_sessions = app.max_sessions.unwrap_or(config.max_sessions);
    }
    if let Some(window) = scenario.window {
        if let Some(widths) = window.widths {
            config.widths = widths
                .into_iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(",");
        }
        config.height = window.height.unwrap_or(config.height);
        config.keep_front = window.keep_front.unwrap_or(config.keep_front);
        config.launch_if_missing = window.launch.unwrap_or(config.launch_if_missing);
    }
    if let Some(runtime) = scenario.runtime {
        config.window_timeout = runtime.window_timeout.unwrap_or(config.window_timeout);
        config.trace_timeout = runtime.trace_timeout.unwrap_or(config.trace_timeout);
        config.settle = runtime.settle.unwrap_or(config.settle);
    }
    config.output_dir = output_override.or(scenario.output_dir);
    Ok(config)
}

fn header_config_from_scenario(
    value: Value,
    output_override: Option<String>,
) -> Result<HeaderDebugConfig> {
    let scenario: HeaderScenarioFile = serde_json::from_value(value)?;
    let mut config = HeaderDebugConfig::default();
    if let Some(app) = scenario.app {
        config.app_root = app.root;
        config.provider = app.provider.unwrap_or(config.provider);
        config.instance = app.instance;
        config.session_id = app.session_id;
        config.session_name = app.session_name;
        config.include_hidden = app.include_hidden.unwrap_or(config.include_hidden);
    }
    if let Some(window) = scenario.window {
        if let Some(widths) = window.widths {
            config.widths = widths
                .into_iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(",");
        }
        config.height = window.height.unwrap_or(config.height);
        config.keep_front = window.keep_front.unwrap_or(config.keep_front);
    }
    if let Some(capture) = scenario.capture {
        config.header_height = capture.header_height.unwrap_or(config.header_height);
    }
    if let Some(runtime) = scenario.runtime {
        config.window_timeout = runtime.window_timeout.unwrap_or(config.window_timeout);
        config.trace_timeout = runtime.trace_timeout.unwrap_or(config.trace_timeout);
        config.settle = runtime.settle.unwrap_or(config.settle);
    }
    config.output_dir = output_override.or(scenario.output_dir);
    Ok(config)
}

#[derive(Deserialize)]
struct DebugScenarioFile {
    output_dir: Option<String>,
    app: Option<DebugScenarioApp>,
    window: Option<DebugScenarioWindow>,
    runtime: Option<ScenarioRuntime>,
}

#[derive(Deserialize)]
struct HeaderScenarioFile {
    output_dir: Option<String>,
    app: Option<HeaderScenarioApp>,
    window: Option<HeaderScenarioWindow>,
    capture: Option<HeaderCapture>,
    runtime: Option<ScenarioRuntime>,
}

#[derive(Deserialize)]
struct DebugScenarioApp {
    root: Option<String>,
    provider: Option<Provider>,
    instance: Option<u32>,
    session_id: Option<String>,
    include_hidden: Option<bool>,
    max_sessions: Option<usize>,
}

#[derive(Deserialize)]
struct HeaderScenarioApp {
    root: Option<String>,
    provider: Option<Provider>,
    instance: Option<u32>,
    session_id: Option<String>,
    session_name: Option<String>,
    include_hidden: Option<bool>,
}

#[derive(Deserialize)]
struct DebugScenarioWindow {
    widths: Option<Vec<u32>>,
    height: Option<u32>,
    keep_front: Option<bool>,
    launch: Option<bool>,
}

#[derive(Deserialize)]
struct HeaderScenarioWindow {
    widths: Option<Vec<u32>>,
    height: Option<u32>,
    keep_front: Option<bool>,
}

#[derive(Deserialize)]
struct HeaderCapture {
    header_height: Option<i32>,
}

#[derive(Deserialize)]
struct ScenarioRuntime {
    window_timeout: Option<f64>,
    trace_timeout: Option<f64>,
    settle: Option<f64>,
}

fn resolve_session(
    provider: Provider,
    session_id: Option<&str>,
    session_name: Option<&str>,
    include_hidden: bool,
) -> Result<SessionDetails> {
    if let Some(session_id) = session_id {
        return load_session_details(provider, session_id);
    }

    let mut sessions = load_metadata_sessions(provider)?;
    sessions.sort_by(|left, right| {
        right
            .get("updated_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .cmp(
                left.get("updated_at")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
    });

    if let Some(session_name) = session_name {
        for raw in &sessions {
            if raw.get("name").and_then(Value::as_str) == Some(session_name) {
                let session_id = raw
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("session named {session_name:?} is missing an id"))?;
                return load_session_details(provider, session_id);
            }
        }
        bail!(
            "Session named {session_name:?} was not found for provider {}.",
            provider.as_str()
        );
    }

    for raw in &sessions {
        if !include_hidden && raw.get("hidden").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        if raw
            .get("message_count")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            > 0
        {
            let session_id = raw
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("session is missing an id"))?;
            return load_session_details(provider, session_id);
        }
    }

    bail!(
        "No visible sessions with messages were found for provider {}.",
        provider.as_str()
    );
}

fn load_metadata_sessions(provider: Provider) -> Result<Vec<Value>> {
    let sessions = read_sessions_metadata(provider)?;
    Ok(sessions.into_values().collect())
}

fn load_session_details(provider: Provider, session_id: &str) -> Result<SessionDetails> {
    let session_path = provider_data_dir(provider)?
        .join("sessions")
        .join(format!("{session_id}.json"));
    let raw: Value = serde_json::from_str(&fs::read_to_string(&session_path)?)?;
    let messages = raw
        .get("messages")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    Ok(SessionDetails {
        session_id: raw
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("session file missing id"))?
            .to_string(),
        name: raw
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("session file missing name"))?
            .to_string(),
        updated_at: raw
            .get("updated_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        message_count: messages,
        provider_session_id: raw
            .get(provider.provider_session_field())
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        launched_from: raw
            .get("launched_from")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

fn run_width_session(
    config: &DebugConfig,
    app_root: &PathBuf,
    title: &str,
    log_path: &PathBuf,
    progress_path: &PathBuf,
    desktop_window_id: Option<&str>,
    per_session_launch_mode: bool,
    window_id: &Option<String>,
    interaction_window_id: Option<&str>,
    width: u32,
    session: &SessionEntry,
    log_offset: &mut u64,
    output_dir: &PathBuf,
    report: &mut Report,
) -> Result<Value> {
    let mut launched_pid: Option<i32> = None;
    let mut current_window_id = window_id.clone();
    let mut current_log_offset = *log_offset;
    let mut geometry: Option<x11::WindowGeometry> = None;
    let mut startup_trace = None;
    let mut startup_code_block_traces = Vec::new();

    let result = (|| -> Result<Value> {
        if per_session_launch_mode {
            let startup_log_offset = fs::metadata(log_path)?.len();
            let (new_pid, new_window_id) = launch_targeted_session_window(
                app_root,
                config.provider,
                config.instance,
                title,
                &session.session_id,
                config.keep_front,
                seconds(config.window_timeout),
                Some(progress_path),
                desktop_window_id,
            )?;
            launched_pid = Some(new_pid);
            current_window_id = Some(new_window_id);
            current_log_offset = fs::metadata(log_path)?.len();
            log_line(
                format!(
                    "observing startup session={} messages={} width={width}",
                    session.name, session.message_count
                ),
                Some(progress_path),
            )?;
            let (new_offset, trace, code_blocks) = wait_for_trace_bundle(
                log_path,
                startup_log_offset,
                &session.session_id,
                seconds(config.trace_timeout.max(10.0)),
                seconds(0.5),
            )?;
            startup_trace = Some(trace);
            startup_code_block_traces = code_blocks;
            current_log_offset = new_offset;
        } else if config.session_id.is_some() {
            log_line(
                format!(
                    "observing startup session={} messages={} width={width}",
                    session.name, session.message_count
                ),
                Some(progress_path),
            )?;
        } else {
            log_line(
                format!(
                    "selecting session={} messages={} width={width}",
                    session.name, session.message_count
                ),
                Some(progress_path),
            )?;
            let interaction_window_id = interaction_window_id
                .ok_or_else(|| anyhow!("No interaction window is available for selection."))?;
            x11::select_session(interaction_window_id, &session.name)?;
            thread::sleep(seconds(config.settle));
        }

        let current_window_id =
            current_window_id.ok_or_else(|| anyhow!("No window is available for capture."))?;

        log_line(
            format!("resizing window to width={width} height={}", config.height),
            Some(progress_path),
        )?;
        geometry = Some(if !config.keep_front {
            x11::prepare_window_for_capture(
                &current_window_id,
                width,
                config.height,
                desktop_window_id,
            )?
        } else {
            x11::resize_window(&current_window_id, width, config.height)?
        });
        thread::sleep(seconds(config.settle));

        let (new_log_offset, trace, code_block_traces) = match wait_for_trace_bundle(
            log_path,
            current_log_offset,
            &session.session_id,
            seconds(config.trace_timeout),
            seconds(0.5),
        ) {
            Ok(result) => result,
            Err(_) => {
                if let Some(startup_trace) = startup_trace.clone() {
                    log_line(
                        format!(
                            "resize produced no new ui trace for session={}; reusing startup trace",
                            session.name
                        ),
                        Some(progress_path),
                    )?;
                    (
                        current_log_offset,
                        startup_trace,
                        startup_code_block_traces.clone(),
                    )
                } else {
                    bail!(
                        "No ui_auto_debug trace observed for session {}",
                        session.session_id
                    );
                }
            }
        };
        *log_offset = new_log_offset;

        let screenshot_path = output_dir.join(format!(
            "{}-w{}-{}.png",
            config.provider.as_str(),
            width,
            &session.session_id[..8]
        ));
        log_line(
            format!(
                "capturing screenshot={}",
                screenshot_path.file_name().unwrap().to_string_lossy()
            ),
            Some(progress_path),
        )?;
        x11::capture_window_screenshot(&current_window_id, &screenshot_path)?;

        let geometry = geometry.ok_or_else(|| anyhow!("window geometry missing after resize"))?;
        let ppp = trace
            .get("pixels_per_point")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(1.0);
        let (screenshot_width, screenshot_height) = x11::image_size(&screenshot_path)?;
        let crop_x = (trace
            .get("first_rect_min_x")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0)
            * ppp) as i32
            - geometry.x;
        let crop_y = (trace
            .get("first_rect_min_y")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0)
            * ppp) as i32
            - geometry.y;
        let crop_x = crop_x.max(0);
        let crop_y = crop_y.max(0);
        let max_crop_width = (screenshot_width - crop_x).max(1);
        let max_crop_height = (screenshot_height - crop_y).max(1);
        let crop_width = ((trace
            .get("first_rect_width")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0)
            * ppp) as i32)
            .max(1)
            .min(max_crop_width);
        let crop_height = ((trace
            .get("first_rect_height")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0)
            * ppp) as i32)
            .max(1)
            .min(max_crop_height);
        let metric = x11::crop_metric(&screenshot_path, crop_x, crop_y, crop_width, crop_height)?;

        log_line(
            format!(
                "captured area_width={} overflow={} code_blocks={} text_visible={}",
                trace
                    .get("message_area_available_width")
                    .map(String::as_str)
                    .unwrap_or("n/a"),
                trace
                    .get("max_rendered_overflow")
                    .map(String::as_str)
                    .unwrap_or("n/a"),
                code_block_traces.len(),
                x11::heuristic_text_visible(&metric)
            ),
            Some(progress_path),
        )?;

        report.add_artifact(
            "window_screenshot",
            screenshot_path.display().to_string(),
            Some(format!("debug width {width} session {}", session.name)),
            json!({ "width": width, "session_id": session.session_id }),
        );

        Ok(json!({
            "session_id": session.session_id,
            "session_name": session.name,
            "message_count": session.message_count,
            "window_geometry": geometry,
            "trace": trace,
            "code_block_traces": code_block_traces,
            "screenshot": screenshot_path,
            "crop": {
                "x": crop_x,
                "y": crop_y,
                "width": crop_width,
                "height": crop_height,
            },
            "visual_metric": metric,
            "text_visible_heuristic": x11::heuristic_text_visible(&metric),
        }))
    })();

    if let Some(launched_pid) = launched_pid {
        stop_chatbot_pid(app_root, launched_pid)?;
        wait_for_pid_exit(app_root, launched_pid, seconds(config.window_timeout))?;
    }

    result
}

fn resolve_app_root(raw_path: Option<&str>) -> Result<PathBuf> {
    if let Some(raw_path) = raw_path {
        return expand_path(raw_path);
    }

    for env_name in ["RUST_CHATBOT_APP_ROOT", "RUST_CHATBOT_ROOT"] {
        if let Ok(value) = std::env::var(env_name) {
            return expand_path(&value);
        }
    }

    let sibling = repo_root()
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .join("rust-chatbot");
    if sibling.exists() {
        return sibling
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", sibling.display()));
    }

    bail!("Could not resolve the rust-chatbot app root. Pass --app-root or set RUST_CHATBOT_APP_ROOT.");
}

fn provider_title(provider: Provider, app_root: &Path) -> String {
    let dir_name = app_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if dir_name.is_empty() {
        provider.title_base().to_string()
    } else {
        format!("{} ~ /{dir_name}", provider.title_base())
    }
}

fn provider_data_dir(provider: Provider) -> Result<PathBuf> {
    Ok(home_dir()?.join(provider.data_dir_name()))
}

fn rust_chatbot_log_dir() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("XDG_STATE_HOME") {
        return Ok(PathBuf::from(path).join("rust-chatbot"));
    }
    Ok(home_dir()?
        .join(".local")
        .join("state")
        .join("rust-chatbot"))
}

fn newest_trace_log() -> Result<PathBuf> {
    let log_dir = rust_chatbot_log_dir()?;
    let mut candidates = Vec::new();
    for entry in
        fs::read_dir(&log_dir).with_context(|| format!("failed to read {}", log_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("rust-chatbot.log."))
        {
            candidates.push(path);
        }
    }
    candidates.sort();
    candidates
        .pop()
        .ok_or_else(|| anyhow!("No rust-chatbot tracing log found in {}", log_dir.display()))
}

fn read_sessions_metadata(provider: Provider) -> Result<Map<String, Value>> {
    let metadata_path = provider_data_dir(provider)?.join("sessions.json");
    let payload: Value = serde_json::from_str(
        &fs::read_to_string(&metadata_path)
            .with_context(|| format!("failed to read {}", metadata_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", metadata_path.display()))?;
    payload
        .get("sessions")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "{} does not contain a sessions object",
                metadata_path.display()
            )
        })
}

fn load_sessions(
    provider: Provider,
    max_sessions: usize,
    include_hidden: bool,
    default_session_names: Option<&[&str]>,
) -> Result<Vec<SessionEntry>> {
    let sessions_map = read_sessions_metadata(provider)?;
    let mut sessions = Vec::new();
    let mut seen_names = HashSet::new();

    for raw in sessions_map.values() {
        if !include_hidden && raw.get("hidden").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        if raw
            .get("message_count")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            <= 0
        {
            continue;
        }
        let Some(name) = raw.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !seen_names.insert(name.to_string()) {
            continue;
        }
        let Some(id) = raw.get("id").and_then(Value::as_str) else {
            continue;
        };
        sessions.push(SessionEntry {
            session_id: id.to_string(),
            name: name.to_string(),
            updated_at: raw
                .get("updated_at")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            message_count: raw
                .get("message_count")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        });
    }

    if let Some(default_session_names) = default_session_names {
        let by_name: HashMap<_, _> = sessions
            .into_iter()
            .map(|session| (session.name.clone(), session))
            .collect();
        let mut ordered = Vec::new();
        let mut missing = Vec::new();
        for name in default_session_names {
            if let Some(session) = by_name.get(*name) {
                ordered.push(session.clone());
            } else {
                missing.push((*name).to_string());
            }
        }
        if !missing.is_empty() {
            bail!(
                "Default auto-ui sessions were not found: {}",
                missing.join(", ")
            );
        }
        return Ok(ordered);
    }

    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    sessions.truncate(max_sessions);
    Ok(sessions)
}

fn load_session_by_id(provider: Provider, session_id: &str) -> Result<SessionEntry> {
    let sessions_map = read_sessions_metadata(provider)?;
    let raw = sessions_map.get(session_id).ok_or_else(|| {
        anyhow!(
            "Session {session_id:?} was not found for provider {:?}.",
            provider
        )
    })?;
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Session payload for {session_id} is missing an id"))?;
    let name = raw
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Session payload for {session_id} is missing a name"))?;
    Ok(SessionEntry {
        session_id: id.to_string(),
        name: name.to_string(),
        updated_at: raw
            .get("updated_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        message_count: raw
            .get("message_count")
            .and_then(Value::as_i64)
            .unwrap_or(0),
    })
}

fn require_release_binaries(app_root: &Path, required_binaries: &[&str]) -> Result<()> {
    let missing: Vec<_> = required_binaries
        .iter()
        .map(|binary| app_root.join("target").join("release").join(binary))
        .filter(|path| !path.exists())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let missing_text = missing
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "Missing release binaries required by this tool: {missing_text}. Build them first with the lightweight build path."
    );
}

fn chatbot_ctl_path(app_root: &Path) -> PathBuf {
    app_root.join("target").join("release").join("chatbot-ctl")
}

fn list_chatbot_pids(app_root: &Path) -> Result<HashSet<i32>> {
    require_release_binaries(app_root, &["chatbot-ctl"])?;
    let mut cmd = Command::new(chatbot_ctl_path(app_root));
    cmd.arg("pids");
    let output = run_command(&mut cmd, false)?;
    let mut pids = HashSet::new();
    for line in output.stdout.lines() {
        if let Ok(pid) = line.trim().parse::<i32>() {
            pids.insert(pid);
        }
    }
    Ok(pids)
}

fn launch_window(
    app_root: &Path,
    provider: Provider,
    instance: Option<u32>,
    start_session_id: Option<&str>,
) -> Result<()> {
    require_release_binaries(app_root, &["chatbot-ctl", "rust-chatbot"])?;
    let mut cmd = Command::new(chatbot_ctl_path(app_root));
    cmd.current_dir(app_root);
    cmd.arg("launch").arg("--provider").arg(provider.as_str());
    if let Some(instance) = instance {
        cmd.arg("--instance").arg(instance.to_string());
    }
    cmd.env("RUST_CHATBOT_AUTO_UI_DEBUG", "1");
    request_background_launch(&mut cmd);
    if let Some(session_id) = start_session_id {
        cmd.env("RUST_CHATBOT_START_SESSION_ID", session_id);
    }
    let rendered = auto_ui_core::render_command(&cmd);
    let status = cmd
        .status()
        .with_context(|| format!("failed to run {rendered}"))?;
    if !status.success() {
        bail!("command failed: {rendered}");
    }
    Ok(())
}

fn wait_for_new_pid(
    app_root: &Path,
    before_pids: &HashSet<i32>,
    timeout: Duration,
) -> Result<Option<i32>> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let current = list_chatbot_pids(app_root)?;
        let mut new_pids: Vec<_> = current.difference(before_pids).copied().collect();
        new_pids.sort();
        if let Some(pid) = new_pids.last().copied() {
            return Ok(Some(pid));
        }
        thread::sleep(Duration::from_millis(200));
    }
    Ok(None)
}

fn stop_chatbot_pid(app_root: &Path, pid: i32) -> Result<()> {
    let mut cmd = Command::new(chatbot_ctl_path(app_root));
    cmd.arg("stop").arg(pid.to_string());
    run_command(&mut cmd, false)?;
    Ok(())
}

fn wait_for_pid_exit(app_root: &Path, pid: i32, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !list_chatbot_pids(app_root)?.contains(&pid) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

fn launch_targeted_session_window(
    app_root: &Path,
    provider: Provider,
    instance: Option<u32>,
    title: &str,
    session_id: &str,
    keep_front: bool,
    window_timeout: Duration,
    progress_path: Option<&Path>,
    restore_window_id: Option<&str>,
) -> Result<(i32, String)> {
    let existing_pids = list_chatbot_pids(app_root)?;
    let existing_window_ids: HashSet<_> = x11::find_window_ids(title)?.into_iter().collect();
    log_line(
        format!(
            "launching fresh {title} with RUST_CHATBOT_AUTO_UI_DEBUG=1 session_id={session_id}"
        ),
        progress_path,
    )?;
    launch_window(app_root, provider, instance, Some(session_id))?;
    let launched_pid = wait_for_new_pid(app_root, &existing_pids, window_timeout)?
        .ok_or_else(|| anyhow!("Could not detect a newly launched PID for {title:?}."))?;
    log_line(
        format!("detected launched_pid={launched_pid}"),
        progress_path,
    )?;
    let window_id = x11::find_window_id_for_pid(launched_pid, window_timeout)?
        .or(x11::wait_for_new_window_id(
            title,
            &existing_window_ids,
            window_timeout,
        )?)
        .ok_or_else(|| anyhow!("Could not find a new window matching {title:?} after launch."))?;
    if !keep_front {
        x11::background_window(&window_id, restore_window_id)?;
    }
    log_line(format!("using window_id={window_id}"), progress_path)?;
    Ok((launched_pid, window_id))
}

fn restore_reused_window(
    window_id: &str,
    geometry: &x11::WindowGeometry,
    keep_front: bool,
    restore_window_id: Option<&str>,
    progress_path: Option<&Path>,
) -> Result<()> {
    if !x11::window_exists(window_id)? {
        log_line(
            format!("skipping restore for window_id={window_id} because the window is no longer available"),
            progress_path,
        )?;
        return Ok(());
    }

    log_line(
        format!(
            "restoring window_id={window_id} x={} y={} width={} height={}",
            geometry.x, geometry.y, geometry.width, geometry.height
        ),
        progress_path,
    )?;
    x11::set_window_geometry(window_id, geometry)?;
    if !keep_front {
        x11::background_window(window_id, restore_window_id)?;
    }
    Ok(())
}

fn read_new_lines(log_path: &Path, offset: u64) -> Result<(u64, Vec<String>)> {
    let mut file =
        File::open(log_path).with_context(|| format!("failed to open {}", log_path.display()))?;
    file.seek(SeekFrom::Start(offset))?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;
    let new_offset = file.stream_position()?;
    Ok((new_offset, data.lines().map(ToOwned::to_owned).collect()))
}

fn parse_trace_fields(line: &str) -> TraceFields {
    static FIELD_RE: OnceLock<Regex> = OnceLock::new();
    let re = FIELD_RE.get_or_init(|| Regex::new(r#"(\w+)=((?:"[^"]*")|(?:\S+))"#).unwrap());
    let mut fields = BTreeMap::new();
    for capture in re.captures_iter(line) {
        let key = capture.get(1).unwrap().as_str();
        let mut value = capture.get(2).unwrap().as_str().to_string();
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
        }
        fields.insert(key.to_string(), value);
    }
    fields
}

fn wait_for_trace_bundle(
    log_path: &Path,
    offset: u64,
    session_id: &str,
    timeout: Duration,
    quiet: Duration,
) -> Result<(u64, TraceFields, Vec<TraceFields>)> {
    let deadline = Instant::now() + timeout;
    let mut quiet_deadline: Option<Instant> = None;
    let mut current_offset = offset;
    let mut last_ui_match: Option<TraceFields> = None;
    let mut code_blocks = Vec::new();

    while Instant::now() < deadline {
        let (new_offset, lines) = read_new_lines(log_path, current_offset)?;
        current_offset = new_offset;
        let mut saw_new_relevant_line = false;
        for line in lines {
            if line.contains("ui_auto_debug_code_block") {
                code_blocks.push(parse_trace_fields(&line));
                saw_new_relevant_line = true;
                continue;
            }
            if line.contains("ui_auto_debug") && line.contains(&format!("session_id={session_id}"))
            {
                last_ui_match = Some(parse_trace_fields(&line));
                saw_new_relevant_line = true;
            }
        }

        if last_ui_match.is_some() {
            if saw_new_relevant_line {
                quiet_deadline = Some(Instant::now() + quiet);
            } else if let Some(quiet_deadline) = quiet_deadline {
                if Instant::now() >= quiet_deadline {
                    return Ok((current_offset, last_ui_match.unwrap(), code_blocks));
                }
            }
        }

        thread::sleep(Duration::from_millis(150));
    }

    if let Some(last_ui_match) = last_ui_match {
        return Ok((current_offset, last_ui_match, code_blocks));
    }
    bail!("No ui_auto_debug trace observed for session {session_id}")
}

fn crop_image(source: &Path, target: &Path, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
    let geometry = format!("{width}x{height}+{x}+{y}");
    let mut cmd = Command::new("convert");
    cmd.arg(source)
        .arg("-crop")
        .arg(geometry)
        .arg("+repage")
        .arg(target);
    run_command(&mut cmd, true)?;
    Ok(())
}

fn enhance_image(source: &Path, target: &Path) -> Result<()> {
    let mut cmd = Command::new("convert");
    cmd.arg(source)
        .arg("-colorspace")
        .arg("Gray")
        .arg("-normalize")
        .arg("-contrast-stretch")
        .arg("1%x1%")
        .arg("-resize")
        .arg("200%")
        .arg(target);
    run_command(&mut cmd, true)?;
    Ok(())
}

fn approximate_header_focus_crop(
    trace: &TraceFields,
    geometry: &x11::WindowGeometry,
    screenshot_width: i32,
    screenshot_height: i32,
    header_height: i32,
) -> CropBox {
    let ppp = trace
        .get("pixels_per_point")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(1.0);
    let content_width_px = ((trace
        .get("content_width")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(700.0))
        * ppp) as i32;
    let first_rect_min_x_px = ((trace
        .get("first_rect_min_x")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0))
        * ppp) as i32;
    let mut focus_x = first_rect_min_x_px - geometry.x - 24;
    if focus_x < 0 || focus_x >= screenshot_width {
        focus_x = screenshot_width - content_width_px;
    }
    focus_x = clamp(focus_x, 0, (screenshot_width - 1).max(0));
    let focus_width = clamp(content_width_px.max(1), 1, screenshot_width - focus_x);
    let focus_height = clamp(header_height, 1, screenshot_height);
    CropBox {
        x: focus_x,
        y: 0,
        width: focus_width,
        height: focus_height,
    }
}

fn clamp(value: i32, low: i32, high: i32) -> i32 {
    value.max(low).min(high)
}

fn seconds(value: f64) -> Duration {
    Duration::from_secs_f64(value.max(0.0))
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

    fn provider_from_env() -> Provider {
        match std::env::var("AUTO_UI_TEST_RUST_CHATBOT_PROVIDER")
            .unwrap_or_else(|_| "codex".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "claude" => Provider::Claude,
            "codex" => Provider::Codex,
            "gemini" => Provider::Gemini,
            other => panic!("unsupported AUTO_UI_TEST_RUST_CHATBOT_PROVIDER={other}"),
        }
    }

    #[test]
    #[ignore = "requires DISPLAY, built rust-chatbot binaries, and AUTO_UI_RUN_LIVE_TESTS=1"]
    fn live_debug_single_session_smoke() {
        let _guard = live_test_lock().lock().unwrap();
        require_live_opt_in();
        let output_dir = unique_temp_dir("rust-chatbot-debug-live");
        let completed = run_debug(DebugConfig {
            app_root: Some(require_env("AUTO_UI_TEST_RUST_CHATBOT_ROOT")),
            provider: provider_from_env(),
            instance: None,
            widths: "520".to_string(),
            height: 720,
            max_sessions: 1,
            session_id: Some(require_env("AUTO_UI_TEST_RUST_CHATBOT_SESSION_ID")),
            include_hidden: true,
            launch_if_missing: true,
            window_timeout: 20.0,
            trace_timeout: 10.0,
            settle: 0.7,
            output_dir: Some(output_dir.display().to_string()),
            keep_front: false,
        })
        .unwrap();
        assert!(completed.report_path.exists());
    }

    #[test]
    #[ignore = "requires DISPLAY, built rust-chatbot binaries, and AUTO_UI_RUN_LIVE_TESTS=1"]
    fn live_header_debug_single_session_smoke() {
        let _guard = live_test_lock().lock().unwrap();
        require_live_opt_in();
        let output_dir = unique_temp_dir("rust-chatbot-header-live");
        let completed = run_header_debug(HeaderDebugConfig {
            app_root: Some(require_env("AUTO_UI_TEST_RUST_CHATBOT_ROOT")),
            provider: provider_from_env(),
            instance: None,
            session_id: Some(require_env("AUTO_UI_TEST_RUST_CHATBOT_SESSION_ID")),
            session_name: None,
            include_hidden: true,
            widths: "520".to_string(),
            height: 720,
            header_height: 140,
            window_timeout: 20.0,
            trace_timeout: 12.0,
            settle: 0.8,
            output_dir: Some(output_dir.display().to_string()),
            keep_front: false,
        })
        .unwrap();
        assert!(completed.report_path.exists());
    }
}
