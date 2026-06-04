pub fn run_prompt_debug(driver: &dyn WindowDriver, config: PromptDebugConfig) -> Result<CompletedRun> {
    auto_ui_core::ensure_display("Rust Chatbot")?;

    if config.session_id.trim().is_empty() {
        bail!("prompt_debug requires a non-empty session_id");
    }
    if config.prompt.trim().is_empty() {
        bail!("prompt_debug requires a non-empty prompt");
    }

    let app_root = resolve_app_root(config.app_root.as_deref())?;
    require_release_binaries(&app_root, &["chatbot-ctl", "rust-chatbot"])?;
    let desktop_window_id = driver.get_active_window()?;
    let title = provider_title(config.provider, &app_root);
    let output_dir = build_output_dir(config.output_dir.as_deref(), "auto-ui-prompt-debug")?;
    let progress_path = output_dir.join("progress.log");
    let launch_start_time = std::time::SystemTime::now();
    let mut log_path = newest_trace_log()?;

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
        format!("session_id={}", config.session_id),
        Some(&progress_path),
    )?;

    let mut launched_pid = None;
    let mut report = Report::new(TARGET_ID, "prompt_debug", "hybrid", &app_root);
    report.add_artifact(
        "progress_log",
        progress_path.display().to_string(),
        Some("live progress log".to_string()),
        Value::Null,
    );
    let output_dir_for_error_report = output_dir.clone();
    let result = (|| -> Result<CompletedRun> {
        let (new_pid, window_id) = launch_targeted_session_window(
            driver,
            &app_root,
            config.provider,
            config.instance,
            &title,
            &config.session_id,
            config.keep_front,
            seconds(config.window_timeout),
            Some(&progress_path),
            desktop_window_id.as_deref(),
        )?;
        launched_pid = Some(new_pid);

        // Check if a new log file was created after launch (log rotation)
        // If so, switch to the new log
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
            }
        }

        if config.keep_front {
            driver.resize(&window_id, config.width, config.height)?;
        } else {
            driver.prepare_window_for_capture(
                &window_id,
                config.width,
                config.height,
                desktop_window_id.as_deref(),
            )?;
        }
        thread::sleep(seconds(config.settle));

        let log_offset = fs::metadata(&log_path)?.len();
        send_prompt_to_session(
            &app_root,
            config.provider,
            &config.session_id,
            &config.prompt,
            launched_pid,
        )?;
        log_line(
            "prompt sent via chatbot-ctl send",
            Some(&progress_path),
        )?;

        let prompt_result = wait_for_prompt_result(
            &log_path,
            log_offset,
            &config.session_id,
            seconds(config.response_timeout),
            seconds(2.5),
        )?;

        let screenshot_path = output_dir.join(format!(
            "{}-{}-prompt-window.png",
            config.provider.as_str(),
            &config.session_id[..8.min(config.session_id.len())]
        ));
        driver.screenshot(&window_id, &screenshot_path)?;

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
        report.add_artifact(
            "window_screenshot",
            screenshot_path.display().to_string(),
            Some("captured prompt-debug window".to_string()),
            json!({
                "width": config.width,
                "height": config.height,
            }),
        );
        report.push_measurement(json!({
            "session_id": config.session_id,
            "provider": config.provider.as_str(),
            "width": config.width,
            "height": config.height,
            "prompt_len": config.prompt.len(),
            "ai_response_end": prompt_result.ai_response_end,
            "upgrade_events": prompt_result.upgrade_events,
            "last_markdown_row": prompt_result.last_markdown_row,
        }));
        report.set_details(json!({
            "provider": config.provider.as_str(),
            "session_id": config.session_id,
            "prompt": config.prompt,
            "width": config.width,
            "height": config.height,
        }));
        report.finish_ok();
        let report_path = write_report(&output_dir, &report)?;
        println!("wrote {}", report_path.display());
        println!("  progress: {}", progress_path.display());
        println!("  trace: {}", log_path.display());
        println!("  report: {}", report_path.display());

        Ok(CompletedRun {
            output_dir,
            report_path,
        })
    })();

    // Write report on failure (success path writes it inside the closure)
    if let Err(err) = &result {
        let err_msg = err.to_string();
        report.finish_error(&err_msg);
        if let Err(err) = write_report(&output_dir_for_error_report, &report) {
            let _ = log_line(
                format!("warning: failed to write error report: {err:#}"),
                Some(&progress_path),
            );
        }
    }

    if let Some(launched_pid) = launched_pid {
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

fn prompt_debug_config_from_scenario(
    value: Value,
    output_override: Option<String>,
) -> Result<PromptDebugConfig> {
    let scenario: PromptDebugScenarioFile = serde_json::from_value(value)?;
    let mut config = PromptDebugConfig::default();
    if let Some(app) = scenario.app {
        config.app_root = app.root;
        config.provider = app.provider.unwrap_or(config.provider);
        config.instance = app.instance;
        if let Some(session_id) = app.session_id {
            config.session_id = session_id;
        }
    }
    if let Some(window) = scenario.window {
        config.width = window.width.unwrap_or(config.width);
        config.height = window.height.unwrap_or(config.height);
        config.keep_front = window.keep_front.unwrap_or(config.keep_front);
    }
    if let Some(prompt) = scenario.prompt {
        config.prompt = prompt.text.unwrap_or_default();
    }
    if let Some(runtime) = scenario.runtime {
        config.window_timeout = runtime.window_timeout.unwrap_or(config.window_timeout);
        config.response_timeout = runtime.trace_timeout.unwrap_or(config.response_timeout);
        config.settle = runtime.settle.unwrap_or(config.settle);
    }
    config.output_dir = output_override.or(scenario.output_dir);
    Ok(config)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DebugScenarioFile {
    output_dir: Option<String>,
    app: Option<DebugScenarioApp>,
    window: Option<DebugScenarioWindow>,
    runtime: Option<ScenarioRuntime>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeaderScenarioFile {
    output_dir: Option<String>,
    app: Option<HeaderScenarioApp>,
    window: Option<HeaderScenarioWindow>,
    capture: Option<HeaderCapture>,
    runtime: Option<ScenarioRuntime>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PromptDebugScenarioFile {
    output_dir: Option<String>,
    app: Option<PromptDebugScenarioApp>,
    window: Option<PromptDebugScenarioWindow>,
    prompt: Option<PromptScenarioPrompt>,
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
struct PromptDebugScenarioApp {
    root: Option<String>,
    provider: Option<Provider>,
    instance: Option<u32>,
    session_id: Option<String>,
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
struct PromptDebugScenarioWindow {
    width: Option<u32>,
    height: Option<u32>,
    keep_front: Option<bool>,
}

#[derive(Deserialize)]
struct PromptScenarioPrompt {
    text: Option<String>,
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

