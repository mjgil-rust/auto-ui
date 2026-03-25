use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use clap::Args as ClapArgs;
use serde_json::{json, Value};

use crate::common::{self, Provider, SessionEntry, WindowGeometry};
const DEFAULT_SESSION_NAMES: &[&str] = &[
    "pl-update",
    "pl-enhance",
    "pl-24",
    "pl-assess",
    "da-scrape-result-submission",
    "pl-graph-problem",
    "pl-enhancements-2",
];

#[derive(Clone, Debug, ClapArgs)]
pub struct Args {
    #[arg(long)]
    pub app_root: Option<String>,
    #[arg(long, value_enum, default_value_t = Provider::Codex)]
    pub provider: Provider,
    #[arg(long)]
    pub instance: Option<u32>,
    #[arg(long, default_value = "520,900,1000")]
    pub widths: String,
    #[arg(long, default_value_t = 900)]
    pub height: u32,
    #[arg(long, default_value_t = 8)]
    pub max_sessions: usize,
    #[arg(long)]
    pub session_id: Option<String>,
    #[arg(long)]
    pub include_hidden: bool,
    #[arg(
        long = "launch",
        default_value_t = true,
        action = clap::ArgAction::Set,
        help = "Launch a fresh app window with RUST_CHATBOT_AUTO_UI_DEBUG=1 and use that window."
    )]
    #[arg(
        long = "no-launch",
        action = clap::ArgAction::SetFalse,
        overrides_with = "launch_if_missing",
        help = "Reuse an existing matching window instead of launching a fresh one."
    )]
    pub launch_if_missing: bool,
    #[arg(long, default_value_t = 15.0)]
    pub window_timeout: f64,
    #[arg(long, default_value_t = 5.0)]
    pub trace_timeout: f64,
    #[arg(long, default_value_t = 0.7)]
    pub settle: f64,
    #[arg(long)]
    pub output_dir: Option<String>,
    #[arg(
        long,
        help = "Do not lower launched automation windows behind other windows."
    )]
    pub keep_front: bool,
}

pub fn run(args: Args) -> Result<()> {
    common::ensure_display()?;

    let app_root = common::resolve_app_root(args.app_root.as_deref())?;
    common::require_release_binaries(&app_root, &["chatbot-ctl", "rust-chatbot"])?;
    let desktop_window_id = common::get_active_window_id()?;

    let widths = common::parse_widths(&args.widths)?;
    let sessions = if let Some(session_id) = args.session_id.as_deref() {
        vec![common::load_session_by_id(args.provider, session_id)?]
    } else {
        common::load_sessions(
            args.provider,
            args.max_sessions,
            args.include_hidden,
            Some(DEFAULT_SESSION_NAMES),
        )?
    };
    if sessions.is_empty() {
        bail!("No visible sessions with messages were found for that provider.");
    }

    let title = common::provider_title(args.provider, &app_root);
    let output_dir = common::build_output_dir(args.output_dir.as_deref(), "auto-ui-debug")?;
    let progress_path = output_dir.join("progress.log");
    let log_path = common::newest_trace_log()?;
    let mut log_offset = fs::metadata(&log_path)?.len();

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

    let per_session_launch_mode = args.launch_if_missing && args.session_id.is_none();
    let mut window_id: Option<String> = None;
    let mut interaction_window_id: Option<String> = None;

    if !per_session_launch_mode {
        if args.launch_if_missing {
            let existing_pids = common::list_chatbot_pids(&app_root)?;
            let existing_window_ids: HashSet<_> = common::find_window_ids(&title)?.into_iter().collect();
            let existing_interaction_window_ids: HashSet<_> =
                common::find_interaction_window_ids(&title)?.into_iter().collect();
            common::log_line(
                format!("launching fresh {title} with RUST_CHATBOT_AUTO_UI_DEBUG=1"),
                Some(&progress_path),
            )?;
            common::launch_window(&app_root, args.provider, args.instance, args.session_id.as_deref())?;
            let launched_pid = common::wait_for_new_pid(
                &app_root,
                &existing_pids,
                seconds(args.window_timeout),
            )?;
            if let Some(launched_pid) = launched_pid {
                common::log_line(format!("detected launched_pid={launched_pid}"), Some(&progress_path))?;
                window_id = common::find_window_id_for_pid(launched_pid, seconds(args.window_timeout))?;
                interaction_window_id =
                    common::find_interaction_window_id_for_pid(launched_pid, seconds(args.window_timeout))?;
            }

            if window_id.is_none() {
                window_id = common::wait_for_new_window_id(
                    &title,
                    &existing_window_ids,
                    seconds(args.window_timeout),
                )?;
            }
            let Some(existing_window_id) = window_id.clone() else {
                bail!("Could not find a new window matching {title:?} after launch.");
            };
            if interaction_window_id.is_none() {
                interaction_window_id = common::wait_for_new_interaction_window_id(
                    &title,
                    &existing_interaction_window_ids,
                    seconds(args.window_timeout),
                )?;
            }
            if !args.keep_front {
                common::background_window(&existing_window_id, desktop_window_id.as_deref())?;
            }
        } else {
            window_id = common::find_window_id(&title, seconds(args.window_timeout))?;
            if window_id.is_none() {
                bail!(
                    "No window matching {title:?} was found. Re-run without --no-launch to let the tool start one."
                );
            }
            interaction_window_id =
                common::find_interaction_window_id(&title, seconds(args.window_timeout))?;
        }

        if interaction_window_id.is_none() {
            interaction_window_id = window_id.clone();
        }

        common::log_line(
            format!("using window_id={}", window_id.as_deref().unwrap_or("<none>")),
            Some(&progress_path),
        )?;
        common::log_line(
            format!(
                "using interaction_window_id={}",
                interaction_window_id.as_deref().unwrap_or("<none>")
            ),
            Some(&progress_path),
        )?;
    }

    let mut report_sessions = Vec::new();

    for width in &widths {
        let mut session_entries = Vec::new();

        for session in &sessions {
            match run_width_session(
                &args,
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
            ) {
                Ok(entry) => session_entries.push(entry),
                Err(err) => {
                    common::log_line(
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

    let report = json!({
        "app_root": app_root,
        "provider": args.provider.as_str(),
        "instance": args.instance,
        "window_id": window_id,
        "title_substring": title,
        "widths": widths,
        "height": args.height,
        "sessions": report_sessions,
        "output_dir": output_dir,
        "trace_log": log_path,
    });

    let report_path = output_dir.join("report.json");
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    println!("wrote {}", report_path.display());
    if let Some(width_entries) = report.get("sessions").and_then(Value::as_array) {
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
                            entry.get("session_name").and_then(Value::as_str).unwrap_or("<unknown>"),
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
                        entry.get("session_name").and_then(Value::as_str).unwrap_or("<unknown>"),
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

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_width_session(
    args: &Args,
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
) -> Result<Value> {
    let mut launched_pid: Option<i32> = None;
    let mut current_window_id = window_id.clone();
    let mut current_log_offset = *log_offset;
    let mut geometry: Option<WindowGeometry> = None;
    let mut startup_trace = None;
    let mut startup_code_block_traces = Vec::new();

    let result = (|| -> Result<Value> {
        if per_session_launch_mode {
            let startup_log_offset = fs::metadata(log_path)?.len();
            let (new_pid, new_window_id) = common::launch_targeted_session_window(
                app_root,
                args.provider,
                args.instance,
                title,
                &session.session_id,
                args.keep_front,
                seconds(args.window_timeout),
                Some(progress_path),
                desktop_window_id,
            )?;
            launched_pid = Some(new_pid);
            current_window_id = Some(new_window_id);
            current_log_offset = fs::metadata(log_path)?.len();
            common::log_line(
                format!(
                    "observing startup session={} messages={} width={width}",
                    session.name, session.message_count
                ),
                Some(progress_path),
            )?;
            let (new_offset, trace, code_blocks) = common::wait_for_trace_bundle(
                log_path,
                startup_log_offset,
                &session.session_id,
                seconds(args.trace_timeout.max(10.0)),
                seconds(0.5),
            )?;
            startup_trace = Some(trace);
            startup_code_block_traces = code_blocks;
            current_log_offset = new_offset;
        } else if args.session_id.is_some() {
            common::log_line(
                format!(
                    "observing startup session={} messages={} width={width}",
                    session.name, session.message_count
                ),
                Some(progress_path),
            )?;
        } else {
            common::log_line(
                format!(
                    "selecting session={} messages={} width={width}",
                    session.name, session.message_count
                ),
                Some(progress_path),
            )?;
            let interaction_window_id =
                interaction_window_id.ok_or_else(|| anyhow!("No interaction window is available for selection."))?;
            common::select_session(interaction_window_id, &session.name)?;
            std::thread::sleep(seconds(args.settle));
        }

        let current_window_id =
            current_window_id.ok_or_else(|| anyhow!("No window is available for capture."))?;

        common::log_line(
            format!("resizing window to width={width} height={}", args.height),
            Some(progress_path),
        )?;
        geometry = Some(common::resize_window(&current_window_id, width, args.height)?);
        if args.launch_if_missing && !args.keep_front {
            common::background_window(&current_window_id, desktop_window_id)?;
        }
        std::thread::sleep(seconds(args.settle));

        let (new_log_offset, trace, code_block_traces) =
            match common::wait_for_trace_bundle(
                log_path,
                current_log_offset,
                &session.session_id,
                seconds(args.trace_timeout),
                seconds(0.5),
            ) {
                Ok(result) => result,
                Err(_) => {
                    if let Some(startup_trace) = startup_trace.clone() {
                        common::log_line(
                            format!(
                                "resize produced no new ui trace for session={}; reusing startup trace",
                                session.name
                            ),
                            Some(progress_path),
                        )?;
                        (current_log_offset, startup_trace, startup_code_block_traces.clone())
                    } else {
                        bail!("No ui_auto_debug trace observed for session {}", session.session_id);
                    }
                }
            };
        *log_offset = new_log_offset;

        let screenshot_path = output_dir.join(format!(
            "{}-w{}-{}.png",
            args.provider.as_str(),
            width,
            &session.session_id[..8]
        ));
        common::log_line(
            format!("capturing screenshot={}", screenshot_path.file_name().unwrap().to_string_lossy()),
            Some(progress_path),
        )?;
        common::capture_window_screenshot(&current_window_id, &screenshot_path)?;

        let geometry = geometry.ok_or_else(|| anyhow!("window geometry missing after resize"))?;
        let ppp = trace
            .get("pixels_per_point")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(1.0);
        let (screenshot_width, screenshot_height) = common::image_size(&screenshot_path)?;
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
        let metric = common::crop_metric(&screenshot_path, crop_x, crop_y, crop_width, crop_height)?;

        common::log_line(
            format!(
                "captured area_width={} overflow={} code_blocks={} text_visible={}",
                trace.get("message_area_available_width").map(String::as_str).unwrap_or("n/a"),
                trace.get("max_rendered_overflow").map(String::as_str).unwrap_or("n/a"),
                code_block_traces.len(),
                common::heuristic_text_visible(&metric)
            ),
            Some(progress_path),
        )?;

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
            "text_visible_heuristic": common::heuristic_text_visible(&metric),
        }))
    })();

    if let Some(launched_pid) = launched_pid {
        common::stop_chatbot_pid(app_root, launched_pid)?;
        common::wait_for_pid_exit(app_root, launched_pid, seconds(args.window_timeout))?;
    }

    result
}

fn seconds(value: f64) -> Duration {
    Duration::from_secs_f64(value.max(0.0))
}
