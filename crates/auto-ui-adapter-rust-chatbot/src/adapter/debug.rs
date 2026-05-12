#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
    Gemini,
    Kimi,
    #[value(name = "geminiforge", alias = "gemini-forge", alias = "gemini_forge")]
    #[serde(alias = "gemini-forge", alias = "gemini_forge")]
    GeminiForge,
    #[value(
        name = "geminicliforge",
        alias = "gemini-cli-forge",
        alias = "gemini_cli_forge"
    )]
    #[serde(alias = "gemini-cli-forge", alias = "gemini_cli_forge")]
    GeminiCliForge,
    #[value(name = "minimaxforge", alias = "minimax-forge", alias = "minimax_forge")]
    #[serde(alias = "minimax-forge", alias = "minimax_forge")]
    MiniMaxForge,
    OmniForge,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
            Provider::Gemini => "gemini",
            Provider::Kimi => "kimi",
            Provider::GeminiForge => "geminiforge",
            Provider::GeminiCliForge => "geminicliforge",
            Provider::MiniMaxForge => "minimaxforge",
            Provider::OmniForge => "omniforge",
        }
    }

    pub fn title_base(self) -> &'static str {
        match self {
            Provider::Claude => "Claude Rust Chatbot",
            Provider::Codex => "Codex Rust Chatbot",
            Provider::Gemini => "Gemini Rust Chatbot",
            Provider::Kimi => "Kimi Rust Chatbot",
            Provider::GeminiForge => "GeminiForge Rust Chatbot",
            Provider::GeminiCliForge => "GeminiCliForge Rust Chatbot",
            Provider::MiniMaxForge => "MiniMaxForge Rust Chatbot",
            Provider::OmniForge => "OmniForge Rust Chatbot",
        }
    }

    pub fn data_dir_name(self) -> &'static str {
        match self {
            Provider::Claude => ".claude-desktop",
            Provider::Codex => ".codex-desktop",
            Provider::Gemini => ".gemini-desktop",
            Provider::Kimi => ".kimi-desktop",
            Provider::GeminiForge => ".gemini-forge-desktop",
            Provider::GeminiCliForge => ".gemini-cli-forge-desktop",
            Provider::MiniMaxForge => ".minimax-forge-desktop",
            Provider::OmniForge => ".omniforge-desktop",
        }
    }

    pub fn provider_session_field(self) -> &'static str {
        match self {
            Provider::Claude => "claude_session_id",
            Provider::Codex => "codex_session_id",
            Provider::Gemini => "gemini_session_id",
            Provider::Kimi => "kimi_session_id",
            Provider::GeminiForge => "gemini_session_id",
            Provider::GeminiCliForge => "gemini_session_id",
            Provider::MiniMaxForge => "minimax_session_id",
            Provider::OmniForge => "omniforge_session_id",
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptDebugConfig {
    pub app_root: Option<String>,
    pub provider: Provider,
    pub instance: Option<u32>,
    pub session_id: String,
    pub prompt: String,
    pub width: u32,
    pub height: u32,
    pub window_timeout: f64,
    pub response_timeout: f64,
    pub settle: f64,
    pub output_dir: Option<String>,
    pub keep_front: bool,
}

impl Default for PromptDebugConfig {
    fn default() -> Self {
        Self {
            app_root: None,
            provider: Provider::Codex,
            instance: None,
            session_id: String::new(),
            prompt: String::new(),
            width: 700,
            height: 900,
            window_timeout: 15.0,
            response_timeout: 45.0,
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
            let config = debug_config_from_scenario(value.clone(), None)?;
            // Validate that widths is not empty (at least one width is required)
            if config.widths.is_empty() {
                bail!("At least one width is required.");
            }
            Ok(())
        }
        "header_debug" => {
            header_config_from_scenario(value.clone(), None)?;
            Ok(())
        }
        "prompt_debug" => {
            prompt_debug_config_from_scenario(value.clone(), None)?;
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
        "prompt_debug" => {
            let config = prompt_debug_config_from_scenario(value, output_override)?;
            run_prompt_debug(config)
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
    let launch_start_time = std::time::SystemTime::now();
    let mut log_path = newest_trace_log()?;
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

    // Check if a new log file was created after launch (log rotation)
    // If so, switch to the new log with offset 0
    if let Ok(new_log_path) = newest_trace_log_since(launch_start_time) {
        if new_log_path != log_path {
            log_line(
                format!(
                    "log rotation detected: switching from {} to {}",
                    log_path.display(),
                    new_log_path.display()
                ),
                Some(&progress_path),
            )?;
            log_path = new_log_path;
            log_offset = 0;
        }
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
    let output_dir_for_error_report = output_dir.clone();
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
