use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use clap::Args as ClapArgs;
use serde_json::{json, Value};

use crate::common::{self, Provider, WindowGeometry};

#[derive(Clone, Debug, ClapArgs)]
pub struct Args {
    #[arg(long)]
    pub app_root: Option<String>,
    #[arg(long, value_enum, default_value_t = Provider::Codex)]
    pub provider: Provider,
    #[arg(long)]
    pub instance: Option<u32>,
    #[arg(long)]
    pub session_id: Option<String>,
    #[arg(long)]
    pub session_name: Option<String>,
    #[arg(long)]
    pub include_hidden: bool,
    #[arg(long, default_value = "520,900,1000")]
    pub widths: String,
    #[arg(long, default_value_t = 900)]
    pub height: u32,
    #[arg(long, default_value_t = 140)]
    pub header_height: i32,
    #[arg(long, default_value_t = 15.0)]
    pub window_timeout: f64,
    #[arg(long, default_value_t = 8.0)]
    pub trace_timeout: f64,
    #[arg(long, default_value_t = 0.8)]
    pub settle: f64,
    #[arg(long)]
    pub output_dir: Option<String>,
    #[arg(long, help = "Do not lower the launched window behind other windows.")]
    pub keep_front: bool,
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

pub fn run(args: Args) -> Result<()> {
    common::ensure_display()?;

    let widths = common::parse_widths(&args.widths)?;
    if widths.is_empty() {
        bail!("At least one width is required.");
    }

    let app_root = common::resolve_app_root(args.app_root.as_deref())?;
    common::require_release_binaries(&app_root, &["chatbot-ctl", "rust-chatbot"])?;
    let desktop_window_id = common::get_active_window_id()?;
    let session = resolve_session(
        args.provider,
        args.session_id.as_deref(),
        args.session_name.as_deref(),
        args.include_hidden,
    )?;
    let title = common::provider_title(args.provider, &app_root);
    let output_dir = common::build_output_dir(args.output_dir.as_deref(), "auto-ui-header-debug")?;
    let progress_path = output_dir.join("progress.log");
    let log_path = common::newest_trace_log()?;
    let startup_offset = fs::metadata(&log_path)?.len();

    common::log_line(format!("app_root={}", app_root.display()), Some(&progress_path))?;
    common::log_line(format!("output_dir={}", output_dir.display()), Some(&progress_path))?;
    common::log_line(format!("trace_log={}", log_path.display()), Some(&progress_path))?;
    common::log_line(
        format!(
            "desktop_window_id={}",
            desktop_window_id.as_deref().unwrap_or("<none>")
        ),
        Some(&progress_path),
    )?;
    common::log_line(format!("session_id={}", session.session_id), Some(&progress_path))?;
    common::log_line(format!("session_name={}", session.name), Some(&progress_path))?;
    common::log_line(
        format!(
            "provider_session_id={}",
            session.provider_session_id.as_deref().unwrap_or("<none>")
        ),
        Some(&progress_path),
    )?;
    common::log_line(
        format!(
            "launched_from={}",
            session.launched_from.as_deref().unwrap_or("<none>")
        ),
        Some(&progress_path),
    )?;

    let mut launched_pid = None;
    let result = (|| -> Result<()> {
        let (new_pid, window_id) = common::launch_targeted_session_window(
            &app_root,
            args.provider,
            args.instance,
            &title,
            &session.session_id,
            args.keep_front,
            seconds(args.window_timeout),
            Some(&progress_path),
            desktop_window_id.as_deref(),
        )?;
        launched_pid = Some(new_pid);

        let (mut current_offset, startup_trace, startup_code_blocks) = common::wait_for_trace_bundle(
            &log_path,
            startup_offset,
            &session.session_id,
            seconds(args.trace_timeout.max(10.0)),
            seconds(0.5),
        )?;
        common::log_line(
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
            common::log_line(
                format!("resizing window to width={width} height={}", args.height),
                Some(&progress_path),
            )?;
            let geometry = common::resize_window(&window_id, width, args.height)?;
            if !args.keep_front {
                common::background_window(&window_id, desktop_window_id.as_deref())?;
            }
            std::thread::sleep(seconds(args.settle));

            let (new_offset, trace, code_block_traces) = match common::wait_for_trace_bundle(
                &log_path,
                current_offset,
                &session.session_id,
                seconds(args.trace_timeout),
                seconds(0.5),
            ) {
                Ok(result) => result,
                Err(_) => {
                    common::log_line(
                        format!("resize produced no new ui trace for width={width}; reusing startup trace"),
                        Some(&progress_path),
                    )?;
                    (current_offset, startup_trace.clone(), startup_code_blocks.clone())
                }
            };
            current_offset = new_offset;

            let screenshot_path = output_dir.join(format!(
                "{}-w{}-{}-window.png",
                args.provider.as_str(),
                width,
                &session.session_id[..8]
            ));
            let top_strip_path = output_dir.join(format!(
                "{}-w{}-{}-top-strip.png",
                args.provider.as_str(),
                width,
                &session.session_id[..8]
            ));
            let focus_path = output_dir.join(format!(
                "{}-w{}-{}-header-focus.png",
                args.provider.as_str(),
                width,
                &session.session_id[..8]
            ));
            let focus_enhanced_path = output_dir.join(format!(
                "{}-w{}-{}-header-focus-enhanced.png",
                args.provider.as_str(),
                width,
                &session.session_id[..8]
            ));

            common::capture_window_screenshot(&window_id, &screenshot_path)?;
            let (screenshot_width, screenshot_height) = common::image_size(&screenshot_path)?;
            let top_strip_height = clamp(args.header_height, 1, screenshot_height);
            crop_image(&screenshot_path, &top_strip_path, 0, 0, screenshot_width, top_strip_height)?;

            let focus_crop = approximate_header_focus_crop(
                &trace,
                &geometry,
                screenshot_width,
                screenshot_height,
                args.header_height,
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
                common::crop_metric(&screenshot_path, 0, 0, screenshot_width, top_strip_height)?;
            let focus_metric = common::crop_metric(
                &screenshot_path,
                focus_crop.x,
                focus_crop.y,
                focus_crop.width,
                focus_crop.height,
            )?;

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

            common::log_line(
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

        let report = json!({
            "app_root": app_root,
            "provider": args.provider.as_str(),
            "instance": args.instance,
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
        });

        let report_path = output_dir.join("report.json");
        fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

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
        if let Some(entries) = report.get("widths").and_then(Value::as_array) {
            for entry in entries {
                println!(
                    "  width {}: window={} header={} enhanced={}",
                    entry.get("requested_width").and_then(Value::as_u64).unwrap_or_default(),
                    entry.get("screenshot").and_then(Value::as_str).unwrap_or("<none>"),
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

        Ok(())
    })();

    if let Some(launched_pid) = launched_pid {
        common::stop_chatbot_pid(&app_root, launched_pid)?;
        common::wait_for_pid_exit(&app_root, launched_pid, seconds(args.window_timeout))?;
    }

    result
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
            .cmp(left.get("updated_at").and_then(Value::as_str).unwrap_or_default())
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
        bail!("Session named {session_name:?} was not found for provider {}.", provider.as_str());
    }

    for raw in &sessions {
        if !include_hidden && raw.get("hidden").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        if raw.get("message_count").and_then(Value::as_i64).unwrap_or(0) > 0 {
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
    let sessions = common::read_sessions_metadata(provider)?;
    Ok(sessions.into_values().collect())
}

fn load_session_details(provider: Provider, session_id: &str) -> Result<SessionDetails> {
    let session_path = common::provider_data_dir(provider)?
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
        launched_from: raw.get("launched_from").and_then(Value::as_str).map(ToOwned::to_owned),
    })
}

fn crop_image(source: &Path, target: &Path, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
    let geometry = format!("{width}x{height}+{x}+{y}");
    let mut cmd = std::process::Command::new("convert");
    cmd.arg(source)
        .arg("-crop")
        .arg(geometry)
        .arg("+repage")
        .arg(target);
    common::run_command(&mut cmd, true)?;
    Ok(())
}

fn enhance_image(source: &Path, target: &Path) -> Result<()> {
    let mut cmd = std::process::Command::new("convert");
    cmd.arg(source)
        .arg("-colorspace")
        .arg("Gray")
        .arg("-normalize")
        .arg("-contrast-stretch")
        .arg("1%x1%")
        .arg("-resize")
        .arg("200%")
        .arg(target);
    common::run_command(&mut cmd, true)?;
    Ok(())
}

#[derive(Clone, Debug, serde::Serialize)]
struct CropBox {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn approximate_header_focus_crop(
    trace: &common::TraceFields,
    geometry: &WindowGeometry,
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
