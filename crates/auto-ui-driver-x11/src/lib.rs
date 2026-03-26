use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use auto_ui_core::{render_command, run_command};
use serde::Serialize;

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

pub fn find_interaction_window_id(
    title_substring: &str,
    timeout: Duration,
) -> Result<Option<String>> {
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

pub fn wait_for_new_interaction_window_id(
    title_substring: &str,
    before_ids: &HashSet<String>,
    timeout: Duration,
) -> Result<Option<String>> {
    wait_for_option(timeout, || {
        let ids = find_interaction_window_ids(title_substring)?;
        Ok(ids
            .into_iter()
            .find(|window_id| !before_ids.contains(window_id)))
    })
}

pub fn wait_for_new_window_id(
    title_substring: &str,
    before_ids: &HashSet<String>,
    timeout: Duration,
) -> Result<Option<String>> {
    wait_for_option(timeout, || {
        let ids = find_window_ids(title_substring)?;
        Ok(ids
            .into_iter()
            .find(|window_id| !before_ids.contains(window_id)))
    })
}

pub fn get_window_geometry(window_id: &str) -> Result<WindowGeometry> {
    let mut cmd = Command::new("xdotool");
    cmd.arg("getwindowgeometry").arg("--shell").arg(window_id);
    let output = run_command(&mut cmd, true)?;
    let mut values = std::collections::HashMap::new();
    for line in output.stdout.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if matches!(key, "X" | "Y" | "WIDTH" | "HEIGHT") {
            values.insert(key.to_string(), value.parse::<i32>()?);
        }
    }
    Ok(WindowGeometry {
        x: *values
            .get("X")
            .ok_or_else(|| anyhow!("missing X geometry field"))?,
        y: *values
            .get("Y")
            .ok_or_else(|| anyhow!("missing Y geometry field"))?,
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
        return Err(anyhow!("Window {window_id} is no longer available."));
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

pub fn set_window_geometry(window_id: &str, geometry: &WindowGeometry) -> Result<WindowGeometry> {
    require_window(window_id)?;

    let mut size_cmd = Command::new("xdotool");
    size_cmd
        .arg("windowsize")
        .arg("--sync")
        .arg(window_id)
        .arg(geometry.width.max(1).to_string())
        .arg(geometry.height.max(1).to_string());
    run_command(&mut size_cmd, true)?;

    let mut move_cmd = Command::new("xdotool");
    move_cmd
        .arg("windowmove")
        .arg("--sync")
        .arg(window_id)
        .arg(geometry.x.to_string())
        .arg(geometry.y.to_string());
    run_command(&mut move_cmd, true)?;

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

pub fn capture_window_screenshot(window_id: &str, path: &Path) -> Result<()> {
    require_window(window_id)?;
    let mut cmd = Command::new("import");
    cmd.arg("-window").arg(window_id).arg(path);
    run_command(&mut cmd, true)?;
    Ok(())
}

pub fn crop_metric(
    image_path: &Path,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<VisualMetric> {
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
    identify_cmd
        .arg("-format")
        .arg("%k")
        .arg(format!("{}[{geometry}]", image_path.display()));
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
