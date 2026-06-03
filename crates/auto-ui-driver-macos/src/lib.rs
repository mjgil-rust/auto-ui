use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Result};
use auto_ui_core::{VisualMetric, WindowDriver, WindowGeometry};

/// macOS-specific window driver using CoreGraphics and Accessibility APIs.
pub struct MacOsWindowDriver;

impl MacOsWindowDriver {
    pub fn new() -> Self {
        Self
    }
}

impl WindowDriver for MacOsWindowDriver {
    fn check_required_tools(&self) -> Result<()> {
        // macOS requires Accessibility permissions instead of external tools.
        // TODO(Phase 4): Implement AXIsProcessTrustedWithOptions check.
        Ok(())
    }

    fn find_windows(&self, _title: &str) -> Result<Vec<String>> {
        bail!("find_windows not yet implemented for macOS")
    }

    fn find_windows_for_pid(&self, _pid: i32) -> Result<Vec<String>> {
        bail!("find_windows_for_pid not yet implemented for macOS")
    }

    fn find_window(&self, _title: &str, _timeout: Duration) -> Result<Option<String>> {
        bail!("find_window not yet implemented for macOS")
    }

    fn find_window_for_pid(&self, _pid: i32, _timeout: Duration) -> Result<Option<String>> {
        bail!("find_window_for_pid not yet implemented for macOS")
    }

    fn wait_for_new_window(
        &self,
        _title: &str,
        _before_ids: &HashSet<String>,
        _timeout: Duration,
    ) -> Result<Option<String>> {
        bail!("wait_for_new_window not yet implemented for macOS")
    }

    fn get_active_window(&self) -> Result<Option<String>> {
        bail!("get_active_window not yet implemented for macOS")
    }

    fn get_window_pid(&self, _window_id: &str) -> Result<Option<i32>> {
        bail!("get_window_pid not yet implemented for macOS")
    }

    fn window_exists(&self, _window_id: &str) -> Result<bool> {
        bail!("window_exists not yet implemented for macOS")
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
}
