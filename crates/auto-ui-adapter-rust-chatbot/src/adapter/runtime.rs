fn read_sessions_metadata(provider: Provider) -> Result<Map<String, Value>> {
    let metadata_path = provider_data_dir(provider)?.join("sessions.json");
    parse_sessions_metadata_from_path(&metadata_path)
}

/// Parse sessions metadata from a specific file path.
/// Useful for testing with mock files.
fn parse_sessions_metadata_from_path(metadata_path: &Path) -> Result<Map<String, Value>> {
    let payload: Value = serde_json::from_str(
        &fs::read_to_string(metadata_path)
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
    load_sessions_from_map(
        sessions_map,
        max_sessions,
        include_hidden,
        default_session_names,
    )
}

/// Load sessions from an already-parsed sessions map.
/// Useful for testing with mock data.
fn load_sessions_from_map(
    sessions_map: Map<String, Value>,
    max_sessions: usize,
    include_hidden: bool,
    default_session_names: Option<&[&str]>,
) -> Result<Vec<SessionEntry>> {
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
            tracing::warn!(
                "Default auto-ui sessions were not found, falling back to recent: {}",
                missing.join(", ")
            );
        }
        // Fall back to recent sessions when named sessions not fully available
        if ordered.len() < default_session_names.len() {
            let mut all_sessions: Vec<_> = sessions_map
                .values()
                .map(|v| SessionEntry {
                    session_id: v
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    name: v
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    updated_at: v
                        .get("updated_at")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    message_count: v.get("message_count").and_then(Value::as_i64).unwrap_or(0),
                })
                .collect();
            all_sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
            let existing_ids: HashSet<_> = ordered.iter().map(|s| s.session_id.clone()).collect();
            for session in all_sessions {
                if !existing_ids.contains(&session.session_id) && ordered.len() < max_sessions {
                    ordered.push(session);
                }
            }
        }
        return Ok(ordered);
    }

    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    sessions.truncate(max_sessions);
    Ok(sessions)
}

fn load_session_by_id(provider: Provider, session_id: &str) -> Result<SessionEntry> {
    let sessions_map = read_sessions_metadata(provider)?;
    load_session_by_id_from_map(sessions_map, provider, session_id)
}

/// Load a session by ID from an already-parsed sessions map.
/// Useful for testing with mock data.
fn load_session_by_id_from_map(
    sessions_map: Map<String, Value>,
    provider: Provider,
    session_id: &str,
) -> Result<SessionEntry> {
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

/// Load a session by name from an already-parsed sessions map.
/// Useful for testing with mock data.
#[cfg(test)]
fn load_session_by_name_from_map(
    sessions_map: Map<String, Value>,
    provider: Provider,
    session_name: &str,
    include_hidden: bool,
) -> Result<SessionEntry> {
    let candidates: Vec<&Map<String, Value>> = sessions_map
        .values()
        .filter_map(|v| v.as_object())
        .filter(|raw| {
            raw.get("name")
                .and_then(Value::as_str)
                .map(|n| n == session_name)
                .unwrap_or(false)
        })
        .filter(|raw| {
            include_hidden || !raw.get("hidden").and_then(Value::as_bool).unwrap_or(false)
        })
        .collect();

    if candidates.is_empty() {
        bail!(
            "Session named {session_name:?} was not found for provider {:?}.",
            provider
        );
    }
    // If multiple sessions have the same name (shouldn't happen), take the first one
    let raw = candidates[0];
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Session payload for {session_name} is missing an id"))?;
    let name = raw
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Session payload for {session_name} is missing a name"))?;
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
    let not_executable: Vec<_> = required_binaries
        .iter()
        .map(|binary| app_root.join("target").join("release").join(binary))
        .filter(|path| {
            if !path.exists() {
                return true; // missing
            }
            // Check executability on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = std::fs::metadata(path) {
                    return metadata.permissions().mode() & 0o111 == 0;
                }
                true // can't read permissions means treat as not executable
            }
            #[cfg(not(unix))]
            {
                false // on non-Unix, existence check is enough
            }
        })
        .collect();
    if not_executable.is_empty() {
        return Ok(());
    }
    let missing_text = not_executable
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "Missing or non-executable release binaries required by this tool: {missing_text}. Build them first with the lightweight build path."
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
    cmd.env("RUST_CHATBOT_SKIP_BACKUP_SCHEDULER_PREFLIGHT", "1");
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
    // Use check=true so that if chatbot-ctl stop fails (e.g., process already gone
    // or permission denied), we propagate the error rather than silently ignoring it.
    run_command(&mut cmd, true)?;
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
    bail!(
        "Timed out after {:?} waiting for pid {} to exit",
        timeout,
        pid
    )
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
    let launched_pid = wait_for_new_pid(app_root, &existing_pids, window_timeout)?;
    let window_id = match launched_pid {
        Some(launched_pid) => {
            log_line(
                format!("detected launched_pid={launched_pid}"),
                progress_path,
            )?;
            x11::find_window_id_for_pid(launched_pid, window_timeout)?
                .or(x11::wait_for_new_window_id(
                    title,
                    &existing_window_ids,
                    window_timeout,
                )?)
        }
        None => {
            log_line(
                "no new PID detected from chatbot-ctl pids; falling back to window-based launch detection"
                    .to_string(),
                progress_path,
            )?;
            x11::wait_for_new_window_id(title, &existing_window_ids, window_timeout)?
        }
    };
    let window_id = match window_id {
        Some(window_id) => window_id,
        None => {
            let matching_windows = x11::find_window_ids(title)?;
            let active_window = x11::get_active_window_id()?;
            let reused_window = active_window
                .filter(|window_id| matching_windows.iter().any(|candidate| candidate == window_id))
                .or_else(|| matching_windows.last().cloned());
            if let Some(window_id) = reused_window {
                log_line(
                    format!(
                        "no new window detected for {title:?}; reusing existing window_id={window_id}"
                    ),
                    progress_path,
                )?;
                window_id
            } else {
                return Err(anyhow!(
                    "Could not find a new or reusable window matching {title:?} after launch."
                ));
            }
        }
    };
    let launched_pid = match launched_pid {
        Some(launched_pid) => launched_pid,
        None => {
            let recovered = x11::get_window_pid(&window_id)?.ok_or_else(|| {
                anyhow!(
                    "Could not detect a newly launched PID for {title:?}, and could not recover one from window {window_id}."
                )
            })?;
            log_line(
                format!("recovered launched_pid={recovered} from window_id={window_id}"),
                progress_path,
            )?;
            recovered
        }
    };
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

#[derive(Clone, Debug)]
struct PromptResult {
    ai_response_end: Option<TraceFields>,
    upgrade_events: Vec<TraceFields>,
    last_markdown_row: Option<TraceFields>,
}

fn send_prompt_to_session(
    app_root: &Path,
    provider: Provider,
    session_id: &str,
    prompt: &str,
    owner_pid: Option<i32>,
) -> Result<()> {
    let mut cmd = Command::new(chatbot_ctl_path(app_root));
    cmd.current_dir(app_root)
        .arg("send")
        .arg("--provider")
        .arg(provider.as_str())
        .arg("--chat-id")
        .arg(session_id)
        .arg("--text")
        .arg(prompt);
    if let Some(owner_pid) = owner_pid {
        cmd.arg("--owner-pid").arg(owner_pid.to_string());
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

fn wait_for_prompt_result(
    log_path: &Path,
    offset: u64,
    session_id: &str,
    timeout: Duration,
    quiet_after_end: Duration,
) -> Result<PromptResult> {
    let deadline = Instant::now() + timeout;
    let mut current_offset = offset;
    let mut quiet_deadline: Option<Instant> = None;
    let mut ai_response_end: Option<TraceFields> = None;
    let mut upgrade_events: Vec<TraceFields> = Vec::new();
    let mut last_markdown_row: Option<TraceFields> = None;

    while Instant::now() < deadline {
        let (new_offset, lines) = read_new_lines(log_path, current_offset)?;
        current_offset = new_offset;
        let mut saw_relevant = false;

        for line in lines {
            if !line.contains(&format!("session_id=\"{session_id}\""))
                && !line.contains(&format!("session_id={session_id}"))
            {
                continue;
            }
            if line.contains("ai_response_end") {
                ai_response_end = Some(parse_trace_fields(&line));
                quiet_deadline = Some(Instant::now() + quiet_after_end);
                saw_relevant = true;
            } else if line.contains("assistant_markdown_upgrade_latency") {
                upgrade_events.push(parse_trace_fields(&line));
                saw_relevant = true;
            } else if line.contains("message_row_render_time")
                && line.contains("rendered_as_markdown=true")
                && line.contains("is_user=false")
                && line.contains("is_tail_message=true")
            {
                last_markdown_row = Some(parse_trace_fields(&line));
                saw_relevant = true;
            }
        }

        let completion_observed = ai_response_end.is_some() || !upgrade_events.is_empty();
        if completion_observed {
            if saw_relevant {
                quiet_deadline = Some(Instant::now() + quiet_after_end);
            } else if let Some(quiet_deadline) = quiet_deadline {
                if Instant::now() >= quiet_deadline {
                    return Ok(PromptResult {
                        ai_response_end,
                        upgrade_events,
                        last_markdown_row,
                    });
                }
            }
        }

        thread::sleep(Duration::from_millis(150));
    }

    if ai_response_end.is_none() && upgrade_events.is_empty() {
        bail!("Timed out waiting for ai_response_end for session {session_id}");
    }

    Ok(PromptResult {
        ai_response_end,
        upgrade_events,
        last_markdown_row,
    })
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

/// Extract session_id from a trace line, checking both quoted and unquoted forms.
fn extract_session_id_from_line(line: &str) -> Option<String> {
    // Try quoted form first: session_id="abc123"
    static QUOTED_RE: OnceLock<Regex> = OnceLock::new();
    let quoted_re = QUOTED_RE.get_or_init(|| Regex::new(r#"session_id="([^"]+)""#).unwrap());
    if let Some(caps) = quoted_re.captures(line) {
        return Some(caps.get(1).unwrap().as_str().to_string());
    }
    // Try unquoted form: session_id=abc123
    static UNQUOTED_RE: OnceLock<Regex> = OnceLock::new();
    let unquoted_re = UNQUOTED_RE.get_or_init(|| Regex::new(r#"session_id=(\S+)"#).unwrap());
    if let Some(caps) = unquoted_re.captures(line) {
        return Some(caps.get(1).unwrap().as_str().to_string());
    }
    None
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
                // Filter code_block rows by session_id (Task #73)
                if let Some(line_session_id) = extract_session_id_from_line(&line) {
                    if line_session_id == session_id {
                        code_blocks.push(parse_trace_fields(&line));
                        saw_new_relevant_line = true;
                    }
                }
                continue;
            }
            if line.contains("ui_auto_debug") {
                // Use extract_session_id_from_line to handle both quoted and unquoted session_id
                if let Some(line_session_id) = extract_session_id_from_line(&line) {
                    if line_session_id == session_id {
                        last_ui_match = Some(parse_trace_fields(&line));
                        saw_new_relevant_line = true;
                    }
                }
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
