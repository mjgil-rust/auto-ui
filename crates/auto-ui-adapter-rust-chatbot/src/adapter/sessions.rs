#[allow(clippy::too_many_arguments)]
fn run_width_session(
    driver: &dyn WindowDriver,
    config: &DebugConfig,
    app_root: &Path,
    title: &str,
    log_path: &Path,
    progress_path: &Path,
    desktop_window_id: Option<&str>,
    per_session_launch_mode: bool,
    window_id: &Option<String>,
    interaction_window_id: Option<&str>,
    width: u32,
    session: &SessionEntry,
    log_offset: &mut u64,
    output_dir: &Path,
    report: &mut Report,
) -> Result<Value> {
    let mut launched_pid: Option<i32> = None;
    let mut current_window_id = window_id.clone();
    let mut current_log_offset = *log_offset;
    let mut startup_trace = None;
    let mut startup_code_block_traces = Vec::new();

    let result = (|| -> Result<Value> {
        if per_session_launch_mode {
            let startup_log_offset = fs::metadata(log_path)?.len();
            let (new_pid, new_window_id) = launch_targeted_session_window(
                driver,
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
            driver.select_session(interaction_window_id, &session.name)?;
            thread::sleep(seconds(config.settle));
        }

        let current_window_id =
            current_window_id.ok_or_else(|| anyhow!("No window is available for capture."))?;

        log_line(
            format!("resizing window to width={width} height={}", config.height),
            Some(progress_path),
        )?;
        let geometry = if !config.keep_front {
            driver.prepare_window_for_capture(
                &current_window_id,
                width,
                config.height,
                desktop_window_id,
            )?
        } else {
            driver.resize(&current_window_id, width, config.height)?
        };
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
        driver.screenshot(&current_window_id, &screenshot_path)?;

        let ppp = trace
            .get("pixels_per_point")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(1.0);
        let (screenshot_width, screenshot_height) = driver.image_size(&screenshot_path)?;
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
        let metric = driver.crop_metric(&screenshot_path, crop_x, crop_y, crop_width, crop_height)?;

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
                heuristic_text_visible(&metric)
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
            "text_visible_heuristic": heuristic_text_visible(&metric),
        }))
    })();

    if let Some(launched_pid) = launched_pid {
        let stop_result = stop_chatbot_pid(app_root, launched_pid).and_then(|_| {
            wait_for_pid_exit(app_root, launched_pid, seconds(config.window_timeout))
        });
        if result.is_ok() {
            stop_result?;
        } else if let Err(err) = stop_result {
            let _ = log_line(
                format!("warning: failed to stop launched_pid={launched_pid}: {err:#}"),
                Some(progress_path),
            );
        }
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

fn rust_chatbot_data_dir() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("RUST_CHATBOT_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    Ok(home_dir()?.join(".rust-chatbot"))
}

fn provider_data_dir(provider: Provider) -> Result<PathBuf> {
    Ok(rust_chatbot_data_dir()?.join(provider.data_dir_name()))
}

fn rust_chatbot_log_dir() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("XDG_STATE_HOME") {
        return Ok(PathBuf::from(path).join("rust-chatbot"));
    }
    let home = home_dir()?;
    let linux_default = home.join(".local").join("state").join("rust-chatbot");
    // macOS path used by rust-chatbot's debug::log_dir() — see
    // chatbot-worker/src/debug.rs: APP_LOG_DIR_MAC = "RustChatbot" under
    // ~/Library/Logs. auto-ui tests on Linux usually have the XDG dir
    // populated, but on macOS the binary writes to Library/Logs instead.
    // We probe both and pick the one that actually contains a fresh log file
    // (so seeded-but-empty Linux dirs on macOS hosts don't shadow the real
    // macOS path).
    let macos_default = home.join("Library").join("Logs").join("RustChatbot");
    let pick = |dir: &PathBuf| -> Option<PathBuf> {
        let entries = fs::read_dir(dir).ok()?;
        let mut newest: Option<(PathBuf, SystemTime)> = None;
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("rust-chatbot.log."))
            {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if newest.as_ref().is_none_or(|(_, t)| modified > *t) {
                            newest = Some((path, modified));
                        }
                    }
                }
            }
        }
        newest.map(|(p, _)| p)
    };
    let macos_pick = pick(&macos_default);
    let linux_pick = pick(&linux_default);
    match (macos_pick, linux_pick) {
        (Some(macos_log), Some(linux_log)) => {
            // Both have logs: prefer the more recently modified one.
            let macos_mtime = fs::metadata(&macos_log).and_then(|m| m.modified()).ok();
            let linux_mtime = fs::metadata(&linux_log).and_then(|m| m.modified()).ok();
            match (macos_mtime, linux_mtime) {
                (Some(m), Some(l)) if l > m => Ok(linux_default),
                _ => Ok(macos_default),
            }
        }
        (Some(_), None) => Ok(macos_default),
        (None, Some(_)) => Ok(linux_default),
        (None, None) => {
            // Neither has logs; fall back to the platform default so the
            // error message names the right location for the host.
            #[cfg(target_os = "macos")]
            {
                let _ = macos_default;
                Ok(linux_default)
            }
            #[cfg(not(target_os = "macos"))]
            Ok(linux_default)
        }
    }
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

/// Get the newest trace log that was modified after the given reference time.
/// This is used after launch to find the log file that the newly launched
/// window is writing to, rather than just picking the lexicographically newest.
/// Falls back to newest_trace_log() if no log was modified after reference_time.
fn newest_trace_log_since(reference_time: std::time::SystemTime) -> Result<PathBuf> {
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
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if modified > reference_time {
                        candidates.push((path, modified));
                    }
                }
            }
        }
    }
    if candidates.is_empty() {
        // Fall back to any trace log if nothing was modified after reference time
        return newest_trace_log();
    }
    candidates.sort_by_key(|b| std::cmp::Reverse(b.1)); // Sort by modified time, newest first
    Ok(candidates.into_iter().next().unwrap().0)
}
