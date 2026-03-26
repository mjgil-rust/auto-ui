use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Local;
use clap::ValueEnum;
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};

pub type TraceFields = BTreeMap<String, String>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
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

#[derive(Clone, Debug, Serialize)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct VisualMetric {
    pub stddev: f64,
    pub colors: f64,
}

pub struct CommandOutput {
    pub stdout: String,
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate inside workspace")
        .to_path_buf()
}

pub fn run_command(command: &mut Command, check: bool) -> Result<CommandOutput> {
    let rendered = render_command(command);
    let output = command
        .output()
        .with_context(|| format!("failed to run {rendered}"))?;
    decode_output(output, &rendered, check)
}

fn decode_output(output: Output, rendered: &str, check: bool) -> Result<CommandOutput> {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if check && !output.status.success() {
        bail!(
            "command failed: {rendered}\nstdout:\n{}\nstderr:\n{}",
            stdout.trim_end(),
            stderr.trim_end()
        );
    }
    Ok(CommandOutput { stdout })
}

fn render_command(command: &Command) -> String {
    let mut parts = Vec::new();
    parts.push(command.get_program().to_string_lossy().into_owned());
    for arg in command.get_args() {
        parts.push(arg.to_string_lossy().into_owned());
    }
    parts.join(" ")
}

pub fn log_line(message: impl AsRef<str>, progress_path: Option<&Path>) -> Result<()> {
    let message = message.as_ref();
    println!("{message}");
    std::io::stdout().flush().ok();
    if let Some(progress_path) = progress_path {
        let mut fh = OpenOptions::new()
            .create(true)
            .append(true)
            .open(progress_path)
            .with_context(|| format!("failed to open {}", progress_path.display()))?;
        writeln!(fh, "{message}").with_context(|| format!("failed to write {}", progress_path.display()))?;
    }
    Ok(())
}

pub fn ensure_display() -> Result<()> {
    if env::var_os("DISPLAY").is_none() {
        bail!("DISPLAY is not set. Run this from a desktop terminal in the same X session as Rust Chatbot.");
    }
    Ok(())
}

pub fn resolve_app_root(raw_path: Option<&str>) -> Result<PathBuf> {
    if let Some(raw_path) = raw_path {
        return expand_path(raw_path);
    }

    for env_name in ["RUST_CHATBOT_APP_ROOT", "RUST_CHATBOT_ROOT"] {
        if let Ok(value) = env::var(env_name) {
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

pub fn provider_title(provider: Provider, app_root: &Path) -> String {
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

pub fn provider_data_dir(provider: Provider) -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(provider.data_dir_name()))
}

pub fn rust_chatbot_log_dir() -> Result<PathBuf> {
    if let Ok(path) = env::var("XDG_STATE_HOME") {
        return Ok(PathBuf::from(path).join("rust-chatbot"));
    }
    Ok(home_dir()?.join(".local").join("state").join("rust-chatbot"))
}

pub fn newest_trace_log() -> Result<PathBuf> {
    let log_dir = rust_chatbot_log_dir()?;
    let mut candidates = Vec::new();
    for entry in fs::read_dir(&log_dir).with_context(|| format!("failed to read {}", log_dir.display()))? {
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

pub fn load_sessions(
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
            bail!("Default auto-ui sessions were not found: {}", missing.join(", "));
        }
        return Ok(ordered);
    }

    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    sessions.truncate(max_sessions);
    Ok(sessions)
}

pub fn load_session_by_id(provider: Provider, session_id: &str) -> Result<SessionEntry> {
    let sessions_map = read_sessions_metadata(provider)?;
    let raw = sessions_map
        .get(session_id)
        .ok_or_else(|| anyhow!("Session {session_id:?} was not found for provider {:?}.", provider))?;
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

pub fn read_sessions_metadata(provider: Provider) -> Result<Map<String, Value>> {
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
        .ok_or_else(|| anyhow!("{} does not contain a sessions object", metadata_path.display()))
}

pub fn require_release_binaries(app_root: &Path, required_binaries: &[&str]) -> Result<()> {
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

pub fn chatbot_ctl_path(app_root: &Path) -> PathBuf {
    app_root.join("target").join("release").join("chatbot-ctl")
}

pub fn list_chatbot_pids(app_root: &Path) -> Result<HashSet<i32>> {
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

pub fn launch_window(
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
    if let Some(session_id) = start_session_id {
        cmd.env("RUST_CHATBOT_START_SESSION_ID", session_id);
    }
    let rendered = render_command(&cmd);
    let status = cmd
        .status()
        .with_context(|| format!("failed to run {rendered}"))?;
    if !status.success() {
        bail!("command failed: {rendered}");
    }
    Ok(())
}

pub fn find_interaction_window_ids(title_substring: &str) -> Result<Vec<String>> {
    let mut cmd = Command::new("xdotool");
    cmd.arg("search")
        .arg("--name")
        .arg(regex::escape(title_substring));
    let output = run_command(&mut cmd, false)?;
    Ok(output
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

pub fn find_interaction_window_ids_for_pid(pid: i32) -> Result<Vec<String>> {
    let mut cmd = Command::new("xdotool");
    cmd.arg("search").arg("--pid").arg(pid.to_string());
    let output = run_command(&mut cmd, false)?;
    Ok(output
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

pub fn find_window_ids(title_substring: &str) -> Result<Vec<String>> {
    let mut cmd = Command::new("wmctrl");
    cmd.arg("-l");
    let output = run_command(&mut cmd, false)?;
    let mut window_ids = Vec::new();
    for line in output.stdout.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 4 {
            continue;
        }
        let title = parts[3..].join(" ");
        if !title.contains(title_substring) {
            continue;
        }
        let id = i64::from_str_radix(parts[0].trim_start_matches("0x"), 16)
            .with_context(|| format!("failed to parse wmctrl id {}", parts[0]))?;
        window_ids.push(id.to_string());
    }
    Ok(window_ids)
}

pub fn find_window_ids_for_pid(pid: i32) -> Result<Vec<String>> {
    let mut cmd = Command::new("wmctrl");
    cmd.arg("-lp");
    let output = run_command(&mut cmd, false)?;
    let mut window_ids = Vec::new();
    for line in output.stdout.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 4 || parts[2] != pid.to_string() {
            continue;
        }
        let id = i64::from_str_radix(parts[0].trim_start_matches("0x"), 16)
            .with_context(|| format!("failed to parse wmctrl id {}", parts[0]))?;
        window_ids.push(id.to_string());
    }
    Ok(window_ids)
}

pub fn find_interaction_window_id(title_substring: &str, timeout: Duration) -> Result<Option<String>> {
    wait_for_option(timeout, || {
        let ids = find_interaction_window_ids(title_substring)?;
        Ok(ids.last().cloned())
    })
}

pub fn find_interaction_window_id_for_pid(pid: i32, timeout: Duration) -> Result<Option<String>> {
    wait_for_option(timeout, || {
        let ids = find_interaction_window_ids_for_pid(pid)?;
        Ok(ids.last().cloned())
    })
}

pub fn find_window_id(title_substring: &str, timeout: Duration) -> Result<Option<String>> {
    wait_for_option(timeout, || {
        let ids = find_window_ids(title_substring)?;
        Ok(ids.last().cloned())
    })
}

pub fn find_window_id_for_pid(pid: i32, timeout: Duration) -> Result<Option<String>> {
    wait_for_option(timeout, || {
        let ids = find_window_ids_for_pid(pid)?;
        Ok(ids.last().cloned())
    })
}

pub fn wait_for_new_pid(app_root: &Path, before_pids: &HashSet<i32>, timeout: Duration) -> Result<Option<i32>> {
    wait_for_option(timeout, || {
        let current = list_chatbot_pids(app_root)?;
        let mut new_pids: Vec<_> = current.difference(before_pids).copied().collect();
        new_pids.sort();
        Ok(new_pids.last().copied())
    })
}

pub fn stop_chatbot_pid(app_root: &Path, pid: i32) -> Result<()> {
    let mut cmd = Command::new(chatbot_ctl_path(app_root));
    cmd.arg("stop").arg(pid.to_string());
    run_command(&mut cmd, false)?;
    Ok(())
}

pub fn wait_for_pid_exit(app_root: &Path, pid: i32, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !list_chatbot_pids(app_root)?.contains(&pid) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

pub fn launch_targeted_session_window(
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
    let existing_window_ids: HashSet<_> = find_window_ids(title)?.into_iter().collect();
    log_line(
        format!("launching fresh {title} with RUST_CHATBOT_AUTO_UI_DEBUG=1 session_id={session_id}"),
        progress_path,
    )?;
    launch_window(app_root, provider, instance, Some(session_id))?;
    let launched_pid = wait_for_new_pid(app_root, &existing_pids, window_timeout)?
        .ok_or_else(|| anyhow!("Could not detect a newly launched PID for {title:?}."))?;
    log_line(format!("detected launched_pid={launched_pid}"), progress_path)?;
    let window_id = find_window_id_for_pid(launched_pid, window_timeout)?
        .or(wait_for_new_window_id(title, &existing_window_ids, window_timeout)?)
        .ok_or_else(|| anyhow!("Could not find a new window matching {title:?} after launch."))?;
    if !keep_front {
        background_window(&window_id, restore_window_id)?;
    }
    log_line(format!("using window_id={window_id}"), progress_path)?;
    Ok((launched_pid, window_id))
}

pub fn wait_for_new_interaction_window_id(
    title_substring: &str,
    before_ids: &HashSet<String>,
    timeout: Duration,
) -> Result<Option<String>> {
    wait_for_option(timeout, || {
        let ids = find_interaction_window_ids(title_substring)?;
        Ok(ids.into_iter().find(|window_id| !before_ids.contains(window_id)))
    })
}

pub fn wait_for_new_window_id(
    title_substring: &str,
    before_ids: &HashSet<String>,
    timeout: Duration,
) -> Result<Option<String>> {
    wait_for_option(timeout, || {
        let ids = find_window_ids(title_substring)?;
        Ok(ids.into_iter().find(|window_id| !before_ids.contains(window_id)))
    })
}

pub fn get_window_geometry(window_id: &str) -> Result<WindowGeometry> {
    let mut cmd = Command::new("xdotool");
    cmd.arg("getwindowgeometry").arg("--shell").arg(window_id);
    let output = run_command(&mut cmd, true)?;
    let mut values = HashMap::new();
    for line in output.stdout.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if matches!(key, "X" | "Y" | "WIDTH" | "HEIGHT") {
            values.insert(key.to_string(), value.parse::<i32>()?);
        }
    }
    Ok(WindowGeometry {
        x: *values.get("X").ok_or_else(|| anyhow!("missing X geometry field"))?,
        y: *values.get("Y").ok_or_else(|| anyhow!("missing Y geometry field"))?,
        width: *values
            .get("WIDTH")
            .ok_or_else(|| anyhow!("missing WIDTH geometry field"))?,
        height: *values
            .get("HEIGHT")
            .ok_or_else(|| anyhow!("missing HEIGHT geometry field"))?,
    })
}

pub fn get_active_window_id() -> Result<Option<String>> {
    let mut cmd = Command::new("xdotool");
    cmd.arg("getactivewindow");
    let output = run_command(&mut cmd, false)?;
    let window_id = output.stdout.trim();
    if window_id.is_empty() {
        Ok(None)
    } else {
        Ok(Some(window_id.to_string()))
    }
}

pub fn window_exists(window_id: &str) -> Result<bool> {
    let mut cmd = Command::new("xdotool");
    cmd.arg("getwindowpid").arg(window_id);
    let rendered = render_command(&cmd);
    let output = cmd
        .output()
        .with_context(|| format!("failed to run {rendered}"))?;
    Ok(output.status.success())
}

pub fn require_window(window_id: &str) -> Result<()> {
    if !window_exists(window_id)? {
        bail!("Window {window_id} is no longer available.");
    }
    Ok(())
}

pub fn activate_window(window_id: &str) -> Result<()> {
    require_window(window_id)?;
    let mut cmd = Command::new("xdotool");
    cmd.arg("windowactivate").arg("--sync").arg(window_id);
    run_command(&mut cmd, true)?;
    Ok(())
}

pub fn lower_window(window_id: &str) -> Result<()> {
    let mut remove_above = Command::new("wmctrl");
    remove_above.args(["-i", "-r", window_id, "-b", "remove,above"]);
    run_command(&mut remove_above, false)?;

    let mut add_below = Command::new("wmctrl");
    add_below.args(["-i", "-r", window_id, "-b", "add,below"]);
    run_command(&mut add_below, false)?;
    Ok(())
}

pub fn background_window(window_id: &str, restore_window_id: Option<&str>) -> Result<()> {
    require_window(window_id)?;
    lower_window(window_id)?;
    if let Some(restore_window_id) = restore_window_id {
        if restore_window_id != window_id && window_exists(restore_window_id)? {
            activate_window(restore_window_id)?;
        }
    }
    Ok(())
}

pub fn resize_window(window_id: &str, width: u32, height: u32) -> Result<WindowGeometry> {
    require_window(window_id)?;
    let mut cmd = Command::new("xdotool");
    cmd.arg("windowsize")
        .arg("--sync")
        .arg(window_id)
        .arg(width.to_string())
        .arg(height.to_string());
    run_command(&mut cmd, true)?;
    get_window_geometry(window_id)
}

pub fn prepare_window_for_capture(
    window_id: &str,
    width: u32,
    height: u32,
    restore_window_id: Option<&str>,
) -> Result<WindowGeometry> {
    let geometry = resize_window(window_id, width, height)?;
    background_window(window_id, restore_window_id)?;
    Ok(geometry)
}

pub fn press_key(window_id: &str, key: &str) -> Result<()> {
    require_window(window_id)?;
    let mut cmd = Command::new("xdotool");
    cmd.args(["key", "--window", window_id, "--clearmodifiers", key]);
    run_command(&mut cmd, true)?;
    Ok(())
}

pub fn clear_search(window_id: &str) -> Result<()> {
    press_key(window_id, "ctrl+a")?;
    press_key(window_id, "BackSpace")
}

pub fn type_text(window_id: &str, text: &str) -> Result<()> {
    require_window(window_id)?;
    let mut cmd = Command::new("xdotool");
    cmd.arg("type")
        .arg("--window")
        .arg(window_id)
        .arg("--delay")
        .arg("1")
        .arg("--clearmodifiers")
        .arg(text);
    run_command(&mut cmd, true)?;
    Ok(())
}

pub fn select_session(window_id: &str, session_name: &str) -> Result<()> {
    press_key(window_id, "ctrl+f")?;
    thread::sleep(Duration::from_millis(100));
    clear_search(window_id)?;
    type_text(window_id, session_name)?;
    thread::sleep(Duration::from_millis(100));
    press_key(window_id, "Return")
}

pub fn read_new_lines(log_path: &Path, offset: u64) -> Result<(u64, Vec<String>)> {
    let mut file = File::open(log_path).with_context(|| format!("failed to open {}", log_path.display()))?;
    file.seek(SeekFrom::Start(offset))?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;
    let new_offset = file.stream_position()?;
    Ok((new_offset, data.lines().map(ToOwned::to_owned).collect()))
}

pub fn parse_trace_fields(line: &str) -> TraceFields {
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

pub fn wait_for_trace_bundle(
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
            if line.contains("ui_auto_debug") && line.contains(&format!("session_id={session_id}")) {
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

pub fn capture_window_screenshot(window_id: &str, path: &Path) -> Result<()> {
    require_window(window_id)?;
    let mut cmd = Command::new("import");
    cmd.arg("-window").arg(window_id).arg(path);
    run_command(&mut cmd, true)?;
    Ok(())
}

pub fn crop_metric(image_path: &Path, x: i32, y: i32, width: i32, height: i32) -> Result<VisualMetric> {
    let geometry = format!("{width}x{height}+{x}+{y}");

    let mut stddev_cmd = Command::new("convert");
    stddev_cmd
        .arg(image_path)
        .arg("-crop")
        .arg(&geometry)
        .arg("+repage")
        .arg("-colorspace")
        .arg("Gray")
        .arg("-format")
        .arg("%[fx:standard_deviation]")
        .arg("info:");
    let stddev = run_command(&mut stddev_cmd, true)?
        .stdout
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0);

    let mut identify_cmd = Command::new("identify");
    identify_cmd.arg("-format").arg("%k").arg(format!("{}[{geometry}]", image_path.display()));
    let colors = run_command(&mut identify_cmd, true)?
        .stdout
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0);

    Ok(VisualMetric { stddev, colors })
}

pub fn image_size(image_path: &Path) -> Result<(i32, i32)> {
    let mut cmd = Command::new("identify");
    cmd.arg("-format").arg("%w %h").arg(image_path);
    let output = run_command(&mut cmd, true)?;
    let mut parts = output.stdout.split_whitespace();
    let width = parts
        .next()
        .ok_or_else(|| anyhow!("identify did not return width"))?
        .parse::<i32>()?;
    let height = parts
        .next()
        .ok_or_else(|| anyhow!("identify did not return height"))?
        .parse::<i32>()?;
    Ok((width, height))
}

pub fn heuristic_text_visible(metric: &VisualMetric) -> bool {
    metric.stddev >= 0.01 || metric.colors >= 16.0
}

pub fn parse_widths(raw: &str) -> Result<Vec<u32>> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u32>().with_context(|| format!("invalid width {part}")))
        .collect()
}

pub fn build_output_dir(output_dir: Option<&str>, prefix: &str) -> Result<PathBuf> {
    let path = if let Some(output_dir) = output_dir {
        expand_path(output_dir)?
    } else {
        repo_root()
            .join("tmp")
            .join(format!("{prefix}-{}", Local::now().format("%Y%m%d-%H%M%S")))
    };
    fs::create_dir_all(&path).with_context(|| format!("failed to create {}", path.display()))?;
    path.canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))
}

fn wait_for_option<T, F>(timeout: Duration, mut f: F) -> Result<Option<T>>
where
    F: FnMut() -> Result<Option<T>>,
{
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(value) = f()? {
            return Ok(Some(value));
        }
        thread::sleep(Duration::from_millis(200));
    }
    Ok(None)
}

fn expand_path(raw_path: &str) -> Result<PathBuf> {
    let path = if raw_path == "~" {
        home_dir()?
    } else if let Some(stripped) = raw_path.strip_prefix("~/") {
        home_dir()?.join(stripped)
    } else {
        PathBuf::from(raw_path)
    };
    path.canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))
}

fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set"))
}
