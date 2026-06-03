use std::collections::HashSet;
use std::ffi::c_void;
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Result};
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

impl WindowDriver for MacOsWindowDriver {
    fn check_required_tools(&self) -> Result<()> {
        // macOS requires Accessibility permissions instead of external tools.
        // TODO(Phase 4): Implement AXIsProcessTrustedWithOptions check.
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
        bail!("get_active_window not yet implemented for macOS")
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

    fn get_geometry(&self, _window_id: &str) -> Result<WindowGeometry> {
        bail!("get_geometry not yet implemented for macOS")
    }

    fn resize(&self, _window_id: &str, _width: u32, _height: u32) -> Result<WindowGeometry> {
        bail!("resize not yet implemented for macOS")
    }

    fn set_geometry(&self, _window_id: &str, _geometry: &WindowGeometry) -> Result<WindowGeometry> {
        bail!("set_geometry not yet implemented for macOS")
    }

    fn activate(&self, _window_id: &str) -> Result<()> {
        bail!("activate not yet implemented for macOS")
    }

    fn lower(&self, _window_id: &str) -> Result<()> {
        bail!("lower not yet implemented for macOS")
    }

    fn background(&self, _window_id: &str, _restore_window_id: Option<&str>) -> Result<()> {
        bail!("background not yet implemented for macOS")
    }

    fn prepare_window_for_capture(
        &self,
        _window_id: &str,
        _width: u32,
        _height: u32,
        _restore_window_id: Option<&str>,
    ) -> Result<WindowGeometry> {
        bail!("prepare_window_for_capture not yet implemented for macOS")
    }

    fn press_key(&self, _window_id: &str, _key: &str) -> Result<()> {
        bail!("press_key not yet implemented for macOS")
    }

    fn type_text(&self, _window_id: &str, _text: &str) -> Result<()> {
        bail!("type_text not yet implemented for macOS")
    }

    fn clear_search(&self, _window_id: &str) -> Result<()> {
        bail!("clear_search not yet implemented for macOS")
    }

    fn select_session(&self, _window_id: &str, _session_name: &str) -> Result<()> {
        bail!("select_session not yet implemented for macOS")
    }

    fn screenshot(&self, _window_id: &str, _output_path: &Path) -> Result<()> {
        bail!("screenshot not yet implemented for macOS")
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
