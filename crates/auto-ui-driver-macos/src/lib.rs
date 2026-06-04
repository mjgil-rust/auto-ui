#![cfg(target_os = "macos")]

use std::collections::HashSet;
use std::ffi::c_void;
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use auto_ui_core::{VisualMetric, WindowDriver, WindowGeometry};
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::window::{
    copy_window_info, kCGWindowListOptionAll, kCGWindowName, kCGWindowNumber, kCGWindowOwnerPID,
};

/// macOS-specific window driver using CoreGraphics and Accessibility APIs.
pub struct MacOsWindowDriver;

impl MacOsWindowDriver {
    pub fn new() -> Self {
        Self
    }
}

/// Retrieve all window dictionaries from CoreGraphics.
fn window_list() -> Vec<CFDictionary<CFString, CFType>> {
    let Some(array) = copy_window_info(kCGWindowListOptionAll, 0) else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|item| {
            let ptr: *const c_void = *item;
            let dict: CFDictionary<CFString, CFType> = unsafe { TCFType::wrap_under_get_rule(ptr.cast()) };
            Some(dict)
        })
        .collect()
}

/// Extract a string value from a window dictionary.
fn get_string(dict: &CFDictionary<CFString, CFType>, key: CFString) -> Option<String> {
    dict.find(&key).map(|value| {
        let s: CFString = unsafe { TCFType::wrap_under_get_rule(value.as_concrete_TypeRef().cast()) };
        s.to_string()
    })
}

/// Extract an i64 value from a window dictionary.
fn get_i64(dict: &CFDictionary<CFString, CFType>, key: CFString) -> Option<i64> {
    dict.find(&key).and_then(|value| {
        let num: CFNumber = unsafe { TCFType::wrap_under_get_rule(value.as_concrete_TypeRef().cast()) };
        num.to_i64()
    })
}

/// Run an AppleScript command and return its stdout.
fn run_applescript(script: &str) -> Result<String> {
    use std::process::Command;
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .with_context(|| "failed to run osascript")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("osascript failed: {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

impl WindowDriver for MacOsWindowDriver {
    fn check_required_tools(&self) -> Result<()> {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
        }
        let trusted = unsafe { AXIsProcessTrustedWithOptions(std::ptr::null()) };
        if !trusted {
            bail!(
                "macOS Accessibility permission is not granted. \
                 Open System Settings > Privacy & Security > Accessibility, \
                 and enable this application."
            );
        }
        Ok(())
    }

    fn find_windows(&self, title: &str) -> Result<Vec<String>> {
        let title_lower = title.to_lowercase();
        let windows = window_list();
        let mut matches = Vec::new();
        for dict in windows {
            if let Some(window_name) = get_string(&dict, unsafe { CFString::wrap_under_get_rule(kCGWindowName) }) {
                if window_name.to_lowercase().contains(&title_lower) {
                    if let Some(window_id) = get_i64(&dict, unsafe { CFString::wrap_under_get_rule(kCGWindowNumber) }) {
                        matches.push(window_id.to_string());
                    }
                }
            }
        }
        Ok(matches)
    }

    fn find_windows_for_pid(&self, pid: i32) -> Result<Vec<String>> {
        let windows = window_list();
        let mut matches = Vec::new();
        for dict in windows {
            if let Some(window_pid) = get_i64(&dict, unsafe { CFString::wrap_under_get_rule(kCGWindowOwnerPID) }) {
                if window_pid == pid as i64 {
                    if let Some(window_id) = get_i64(&dict, unsafe { CFString::wrap_under_get_rule(kCGWindowNumber) }) {
                        matches.push(window_id.to_string());
                    }
                }
            }
        }
        Ok(matches)
    }

    fn find_window(&self, title: &str, timeout: Duration) -> Result<Option<String>> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            let windows = self.find_windows(title)?;
            if let Some(id) = windows.last() {
                return Ok(Some(id.clone()));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(None)
    }

    fn find_window_for_pid(&self, pid: i32, timeout: Duration) -> Result<Option<String>> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            let windows = self.find_windows_for_pid(pid)?;
            if let Some(id) = windows.last() {
                return Ok(Some(id.clone()));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(None)
    }

    fn wait_for_new_window(
        &self,
        title: &str,
        before_ids: &HashSet<String>,
        timeout: Duration,
    ) -> Result<Option<String>> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            let windows = self.find_windows(title)?;
            for id in windows {
                if !before_ids.contains(&id) {
                    return Ok(Some(id));
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(None)
    }

    fn get_active_window(&self) -> Result<Option<String>> {
        // Use CoreGraphics to find the frontmost on-screen normal window.
        // This is a heuristic; the most accurate method requires Accessibility APIs.
        let windows = window_list();
        for dict in windows {
            let layer = get_i64(&dict, unsafe { CFString::wrap_under_get_rule(core_graphics::window::kCGWindowLayer) }).unwrap_or(0);
            let onscreen = get_i64(&dict, unsafe { CFString::wrap_under_get_rule(core_graphics::window::kCGWindowIsOnscreen) }).unwrap_or(0);
            if layer == 0 && onscreen == 1 {
                if let Some(id) = get_i64(&dict, unsafe { CFString::wrap_under_get_rule(kCGWindowNumber) }) {
                    return Ok(Some(id.to_string()));
                }
            }
        }
        Ok(None)
    }

    fn get_window_pid(&self, window_id: &str) -> Result<Option<i32>> {
        let target_id: i64 = window_id.parse().ok().unwrap_or(-1);
        let windows = window_list();
        for dict in windows {
            if let Some(id) = get_i64(&dict, unsafe { CFString::wrap_under_get_rule(kCGWindowNumber) }) {
                if id == target_id {
                    return Ok(get_i64(&dict, unsafe { CFString::wrap_under_get_rule(kCGWindowOwnerPID) }).map(|p| p as i32));
                }
            }
        }
        Ok(None)
    }

    fn window_exists(&self, window_id: &str) -> Result<bool> {
        let target_id: i64 = window_id.parse().ok().unwrap_or(-1);
        let windows = window_list();
        for dict in windows {
            if let Some(id) = get_i64(&dict, unsafe { CFString::wrap_under_get_rule(kCGWindowNumber) }) {
                if id == target_id {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn get_geometry(&self, window_id: &str) -> Result<WindowGeometry> {
        let pid = self.get_window_pid(window_id)?
            .ok_or_else(|| anyhow::anyhow!("cannot get geometry: window {window_id} has no PID"))?;
        let script = format!(
            "tell application \"System Events\" to tell (first process whose unix id is {pid}) to tell window 1 to return {{position, size}}"
        );
        let output = run_applescript(&script)?;
        let output = output.trim();
        // Parse output like "{{100, 200}, {800, 600}}"
        let parts: Vec<i32> = output
            .split(|c: char| !c.is_ascii_digit() && c != '-')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap_or(0))
            .collect();
        if parts.len() < 4 {
            bail!("unexpected AppleScript output for geometry: {output}");
        }
        Ok(WindowGeometry {
            x: parts[0],
            y: parts[1],
            width: parts[2],
            height: parts[3],
        })
    }

    fn resize(&self, window_id: &str, width: u32, height: u32) -> Result<WindowGeometry> {
        let pid = self.get_window_pid(window_id)?
            .ok_or_else(|| anyhow::anyhow!("cannot resize: window {window_id} has no PID"))?;
        let script = format!(
            "tell application \"System Events\" to tell (first process whose unix id is {pid}) to set size of window 1 to {{{width}, {height}}}"
        );
        run_applescript(&script)?;
        self.get_geometry(window_id)
    }

    fn set_geometry(&self, window_id: &str, geometry: &WindowGeometry) -> Result<WindowGeometry> {
        let pid = self.get_window_pid(window_id)?
            .ok_or_else(|| anyhow::anyhow!("cannot set geometry: window {window_id} has no PID"))?;
        let script = format!(
            "tell application \"System Events\" to tell (first process whose unix id is {pid}) to tell window 1 to set position to {{{}, {}}} and set size to {{{}, {}}}",
            geometry.x, geometry.y, geometry.width, geometry.height
        );
        run_applescript(&script)?;
        self.get_geometry(window_id)
    }

    fn activate(&self, window_id: &str) -> Result<()> {
        let pid = self.get_window_pid(window_id)?
            .ok_or_else(|| anyhow::anyhow!("cannot activate: window {window_id} has no PID"))?;
        let script = format!(
            "tell application \"System Events\" to set frontmost of first process whose unix id is {pid} to true"
        );
        run_applescript(&script)?;
        Ok(())
    }

    fn lower(&self, _window_id: &str) -> Result<()> {
        // macOS does not have a reliable "lower window" API.
        Ok(())
    }

    fn background(&self, window_id: &str, restore_window_id: Option<&str>) -> Result<()> {
        self.lower(window_id)?;
        if let Some(restore_id) = restore_window_id {
            self.activate(restore_id)?;
        }
        Ok(())
    }

    fn prepare_window_for_capture(
        &self,
        window_id: &str,
        width: u32,
        height: u32,
        restore_window_id: Option<&str>,
    ) -> Result<WindowGeometry> {
        let geometry = self.resize(window_id, width, height)?;
        self.background(window_id, restore_window_id)?;
        Ok(geometry)
    }

    fn press_key(&self, _window_id: &str, key: &str) -> Result<()> {
        use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| anyhow::anyhow!("failed to create CGEventSource"))?;

        // Parse modifiers and key name.
        let parts: Vec<&str> = key.split('+').collect();
        let mut flags = CGEventFlags::CGEventFlagNull;
        let key_name = parts.last().unwrap_or(&"").trim();
        for part in &parts[..parts.len().saturating_sub(1)] {
            match part.trim().to_lowercase().as_str() {
                "ctrl" | "control" => flags |= CGEventFlags::CGEventFlagControl,
                "shift" => flags |= CGEventFlags::CGEventFlagShift,
                "alt" | "option" => flags |= CGEventFlags::CGEventFlagAlternate,
                "cmd" | "command" | "meta" => flags |= CGEventFlags::CGEventFlagCommand,
                _ => {}
            }
        }

        let keycode = match key_name {
            "BackSpace" | "Backspace" | "Delete" => 51,
            "Return" | "Enter" => 36,
            "Escape" | "Esc" => 53,
            "Tab" => 48,
            "Space" => 49,
            "a" | "A" => KeyCode::ANSI_A,
            "b" | "B" => KeyCode::ANSI_B,
            "c" | "C" => KeyCode::ANSI_C,
            "d" | "D" => KeyCode::ANSI_D,
            "e" | "E" => KeyCode::ANSI_E,
            "f" | "F" => KeyCode::ANSI_F,
            "g" | "G" => KeyCode::ANSI_G,
            "h" | "H" => KeyCode::ANSI_H,
            "i" | "I" => KeyCode::ANSI_I,
            "j" | "J" => KeyCode::ANSI_J,
            "k" | "K" => KeyCode::ANSI_K,
            "l" | "L" => KeyCode::ANSI_L,
            "m" | "M" => KeyCode::ANSI_M,
            "n" | "N" => KeyCode::ANSI_N,
            "o" | "O" => KeyCode::ANSI_O,
            "p" | "P" => KeyCode::ANSI_P,
            "q" | "Q" => KeyCode::ANSI_Q,
            "r" | "R" => KeyCode::ANSI_R,
            "s" | "S" => KeyCode::ANSI_S,
            "t" | "T" => KeyCode::ANSI_T,
            "u" | "U" => KeyCode::ANSI_U,
            "v" | "V" => KeyCode::ANSI_V,
            "w" | "W" => KeyCode::ANSI_W,
            "x" | "X" => KeyCode::ANSI_X,
            "y" | "Y" => KeyCode::ANSI_Y,
            "z" | "Z" => KeyCode::ANSI_Z,
            "0" => KeyCode::ANSI_0,
            "1" => KeyCode::ANSI_1,
            "2" => KeyCode::ANSI_2,
            "3" => KeyCode::ANSI_3,
            "4" => KeyCode::ANSI_4,
            "5" => KeyCode::ANSI_5,
            "6" => KeyCode::ANSI_6,
            "7" => KeyCode::ANSI_7,
            "8" => KeyCode::ANSI_8,
            "9" => KeyCode::ANSI_9,
            _ => bail!("unsupported key name: {key_name}"),
        };

        let down = CGEvent::new_keyboard_event(source.clone(), keycode, true)
            .map_err(|_| anyhow::anyhow!("failed to create keydown event"))?;
        if flags != CGEventFlags::CGEventFlagNull {
            down.set_flags(flags);
        }
        down.post(CGEventTapLocation::HID);

        let up = CGEvent::new_keyboard_event(source, keycode, false)
            .map_err(|_| anyhow::anyhow!("failed to create keyup event"))?;
        if flags != CGEventFlags::CGEventFlagNull {
            up.set_flags(flags);
        }
        up.post(CGEventTapLocation::HID);

        Ok(())
    }

    fn type_text(&self, _window_id: &str, text: &str) -> Result<()> {
        use core_graphics::event::{CGEvent, CGEventTapLocation};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| anyhow::anyhow!("failed to create CGEventSource"))?;
        let event = CGEvent::new_keyboard_event(source, 0, true)
            .map_err(|_| anyhow::anyhow!("failed to create keyboard event"))?;
        event.set_string(text);
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn clear_search(&self, window_id: &str) -> Result<()> {
        self.press_key(window_id, "ctrl+a")?;
        self.press_key(window_id, "BackSpace")
    }

    fn select_session(&self, window_id: &str, session_name: &str) -> Result<()> {
        self.press_key(window_id, "ctrl+f")?;
        std::thread::sleep(Duration::from_millis(100));
        self.clear_search(window_id)?;
        self.type_text(window_id, session_name)?;
        std::thread::sleep(Duration::from_millis(100));
        self.press_key(window_id, "Return")
    }

    fn screenshot(&self, window_id: &str, output_path: &Path) -> Result<()> {
        use core_graphics::display::CGRectNull;
        use core_graphics::window::{
            create_image, kCGWindowImageBoundsIgnoreFraming, kCGWindowListOptionIncludingWindow,
        };

        let window_id_num: u32 = window_id.parse().map_err(|_| anyhow::anyhow!("invalid window id: {window_id}"))?;
        let Some(image) = create_image(
            unsafe { CGRectNull },
            kCGWindowListOptionIncludingWindow,
            window_id_num,
            kCGWindowImageBoundsIgnoreFraming,
        ) else {
            bail!("failed to capture screenshot for window {window_id}");
        };

        let width = image.width();
        let height = image.height();
        let bytes_per_row = image.bytes_per_row();
        let data = image.data();
        let raw = data.bytes();

        // CoreGraphics window images are typically BGRA with premultiplied alpha.
        // Convert to RGBA for the image crate.
        let mut rgba = Vec::with_capacity(width * height * 4);
        for row in 0..height {
            let row_start = row * bytes_per_row;
            for col in 0..width {
                let idx = row_start + col * 4;
                let b = raw[idx];
                let g = raw[idx + 1];
                let r = raw[idx + 2];
                let a = raw[idx + 3];
                rgba.push(r);
                rgba.push(g);
                rgba.push(b);
                rgba.push(a);
            }
        }

        let rgba_image = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(width as u32, height as u32, rgba)
            .ok_or_else(|| anyhow::anyhow!("failed to create image buffer from screenshot data"))?;
        rgba_image.save(output_path)
            .with_context(|| format!("failed to save screenshot to {}", output_path.display()))?;
        Ok(())
    }

    fn crop_metric(
        &self,
        image_path: &Path,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<VisualMetric> {
        // Image processing is platform-agnostic; delegate to core.
        auto_ui_core::image_processing::crop_metric(image_path, x, y, width, height)
    }

    fn image_size(&self, image_path: &Path) -> Result<(i32, i32)> {
        // Image processing is platform-agnostic; delegate to core.
        auto_ui_core::image_processing::image_size(image_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_window_driver_implements_window_driver_trait() {
        fn assert_trait<T: WindowDriver>() {}
        assert_trait::<MacOsWindowDriver>();
    }

    #[test]
    fn find_windows_does_not_panic() {
        let driver = MacOsWindowDriver::new();
        let result = driver.find_windows("Finder");
        assert!(result.is_ok());
    }

    #[test]
    fn window_exists_does_not_panic() {
        let driver = MacOsWindowDriver::new();
        let result = driver.window_exists("0");
        assert!(result.is_ok());
    }
}
