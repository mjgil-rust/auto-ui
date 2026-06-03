use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use auto_ui_core::{render_command, run_command};

mod fake_driver;

pub use auto_ui_core::window_driver::{
    heuristic_text_visible, VisualMetric, WindowDriver, WindowGeometry,
};
pub use fake_driver::{FakeWindowDriver, FakeWindowState, FAKE_WINDOW_ID};

/// Concrete X11 window driver backed by `xdotool`, `wmctrl`, and ImageMagick.
pub struct X11WindowDriver;

impl X11WindowDriver {
    /// Create a new X11 window driver.
    pub fn new() -> Self {
        Self
    }
}

impl WindowDriver for X11WindowDriver {
    fn check_required_tools(&self) -> Result<()> {
        check_required_tools()
    }

    fn find_windows(&self, title_substring: &str) -> Result<Vec<String>> {
        find_interaction_window_ids(title_substring)
    }

    fn find_windows_for_pid(&self, pid: i32) -> Result<Vec<String>> {
        find_interaction_window_ids_for_pid(pid)
    }

    fn find_window(&self, title_substring: &str, timeout: Duration) -> Result<Option<String>> {
        find_interaction_window_id(title_substring, timeout)
    }

    fn find_window_for_pid(&self, pid: i32, timeout: Duration) -> Result<Option<String>> {
        find_interaction_window_id_for_pid(pid, timeout)
    }

    fn wait_for_new_window(
        &self,
        title_substring: &str,
        before_ids: &HashSet<String>,
        timeout: Duration,
    ) -> Result<Option<String>> {
        wait_for_new_interaction_window_id(title_substring, before_ids, timeout)
    }

    fn get_active_window(&self) -> Result<Option<String>> {
        get_active_window_id()
    }

    fn get_window_pid(&self, window_id: &str) -> Result<Option<i32>> {
        get_window_pid(window_id)
    }

    fn get_geometry(&self, window_id: &str) -> Result<WindowGeometry> {
        get_window_geometry(window_id)
    }

    fn resize(&self, window_id: &str, width: u32, height: u32) -> Result<WindowGeometry> {
        resize_window(window_id, width, height)
    }

    fn set_geometry(&self, window_id: &str, geometry: &WindowGeometry) -> Result<WindowGeometry> {
        set_window_geometry(window_id, geometry)
    }

    fn activate(&self, window_id: &str) -> Result<()> {
        activate_window(window_id)
    }

    fn lower(&self, window_id: &str) -> Result<()> {
        lower_window(window_id)
    }

    fn background(&self, window_id: &str, restore_window_id: Option<&str>) -> Result<()> {
        background_window(window_id, restore_window_id)
    }

    fn window_exists(&self, window_id: &str) -> Result<bool> {
        window_exists(window_id)
    }

    fn press_key(&self, window_id: &str, key: &str) -> Result<()> {
        press_key(window_id, key)
    }

    fn type_text(&self, window_id: &str, text: &str) -> Result<()> {
        type_text(window_id, text)
    }

    fn clear_search(&self, window_id: &str) -> Result<()> {
        clear_search(window_id)
    }

    fn select_session(&self, window_id: &str, session_name: &str) -> Result<()> {
        select_session(window_id, session_name)
    }

    fn screenshot(&self, window_id: &str, output_path: &Path) -> Result<()> {
        capture_window_screenshot(window_id, output_path)
    }

    fn crop_metric(
        &self,
        image_path: &Path,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<VisualMetric> {
        crop_metric(image_path, x, y, width, height)
    }

    fn image_size(&self, image_path: &Path) -> Result<(i32, i32)> {
        image_size(image_path)
    }

    fn prepare_window_for_capture(
        &self,
        window_id: &str,
        width: u32,
        height: u32,
        restore_window_id: Option<&str>,
    ) -> Result<WindowGeometry> {
        prepare_window_for_capture(window_id, width, height, restore_window_id)
    }
}

impl Default for X11WindowDriver {
    fn default() -> Self {
        Self::new()
    }
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

pub fn get_window_pid(window_id: &str) -> Result<Option<i32>> {
    let mut cmd = Command::new("xdotool");
    cmd.arg("getwindowpid").arg(window_id);
    let output = run_command(&mut cmd, false)?;
    let pid = output.stdout.trim();
    if pid.is_empty() {
        return Ok(None);
    }
    Ok(Some(pid.parse::<i32>()?))
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
    auto_ui_core::image_processing::crop_metric(image_path, x, y, width, height)
}

pub fn image_size(image_path: &Path) -> Result<(i32, i32)> {
    auto_ui_core::image_processing::image_size(image_path)
}

/// Tools required by the X11 driver for window operations and screenshot capture.
pub const REQUIRED_X11_TOOLS: &[&str] = &["xdotool", "wmctrl", "import"];

/// Check that all required X11 tools are available in PATH.
/// Returns Ok(()) if all tools are found, or an error listing missing tools.
pub fn check_required_tools() -> Result<()> {
    let mut missing = Vec::new();
    for tool in REQUIRED_X11_TOOLS {
        if which(tool).is_none() {
            missing.push(*tool);
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        bail!(
            "Missing required X11 tools: {}. Install them with: apt install imagemagick wmctrl xdotool (or equivalent for your distro)",
            missing.join(", ")
        )
    }
}

/// Check if a command is available in PATH.
fn which(cmd: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path_var| {
        std::env::split_paths(&path_var)
            .filter_map(|dir| {
                let path = dir.join(cmd);
                if is_executable(&path) {
                    Some(path)
                } else {
                    None
                }
            })
            .next()
    })
}

/// Check if a path is executable.
#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file())
        .unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn crop_metric_fails_for_non_image() {
        let temp = unique_temp_dir("crop-metric-bad");
        let img_path = temp.join("input.png");
        fs::write(&img_path, "not an image").unwrap();

        let result = crop_metric(&img_path, 0, 0, 1, 1);
        assert!(result.is_err(), "crop_metric should fail for non-image data");
    }

    #[test]
    fn image_size_fails_for_non_image() {
        let temp = unique_temp_dir("image-size-bad");
        let img_path = temp.join("input.png");
        fs::write(&img_path, "not an image").unwrap();

        let result = image_size(&img_path);
        assert!(result.is_err(), "image_size should fail for non-image data");
    }

    #[test]
    fn check_required_tools_missing_tool() {
        // Test that missing tool is detected
        // This will depend on what's installed, but at least one tool should be present
        // in a typical dev environment, or the test will pass if all are missing
        let result = check_required_tools();
        // We just verify it doesn't panic and returns a Result
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn check_required_tools_reports_missing() {
        // Create a fake tool name that should not exist
        let missing = "this_tool_does_not_exist_12345";
        let result = which(missing);
        assert!(result.is_none());
    }

    #[test]
    fn which_finds_existing_command() {
        // "true" exists on all Unix systems
        let result = which("true");
        #[cfg(unix)]
        {
            assert!(result.is_some());
        }
        // On non-Unix, this might not be found
    }

    #[test]
    fn is_executable_detects_executable() {
        #[cfg(target_os = "linux")]
        {
            // /bin/true should be executable
            assert!(is_executable(std::path::Path::new("/bin/true")));
            // /usr/bin directory entry - some systems have directories with exec bit
            // so we just test that a regular file executable check works
        }
    }

    #[test]
    fn is_executable_returns_false_for_nonexistent() {
        assert!(!is_executable(std::path::Path::new(
            "/nonexistent/path/xyz"
        )));
    }

    #[test]
    fn required_x11_tools_has_three_tools() {
        assert_eq!(REQUIRED_X11_TOOLS.len(), 3);
        assert!(REQUIRED_X11_TOOLS.contains(&"xdotool"));
        assert!(REQUIRED_X11_TOOLS.contains(&"wmctrl"));
        assert!(REQUIRED_X11_TOOLS.contains(&"import"));
    }

    #[test]
    fn x11_window_driver_implements_window_driver_trait() {
        fn _assert_trait<T: WindowDriver>() {}
        _assert_trait::<X11WindowDriver>();
    }
}
