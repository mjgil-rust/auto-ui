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
    let output_dir_for_error_report = output_dir.clone();
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

    // Write report on failure (success path writes it inside the closure)
    if result.is_err() {
        if let Err(err) = write_report(&output_dir_for_error_report, &report) {
            let _ = log_line(
                format!("warning: failed to write error report: {err:#}"),
                Some(&progress_path),
            );
        }
    }

    if let Some(launched_pid) = launched_pid {
        stop_chatbot_pid(&app_root, launched_pid)?;
        wait_for_pid_exit(&app_root, launched_pid, seconds(config.window_timeout))?;
    }

    result
}

