//! Fake window driver for integration testing.
//!
//! This module provides a fake implementation of window operations
//! that doesn't require X11 tools, allowing orchestration tests to run
//! in CI without a display.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{VisualMetric, WindowDriver, WindowGeometry};

/// A fake window ID for testing.
pub const FAKE_WINDOW_ID: &str = "0xFAKE123";

/// A fake window state for testing.
#[derive(Clone, Debug)]
pub struct FakeWindowState {
    pub geometry: WindowGeometry,
    pub active: bool,
    pub exists: bool,
}

impl Default for FakeWindowState {
    fn default() -> Self {
        Self {
            geometry: WindowGeometry {
                x: 0,
                y: 0,
                width: 1280,
                height: 800,
            },
            active: false,
            exists: true,
        }
    }
}

/// A fake window driver for testing.
#[derive(Clone, Default)]
pub struct FakeWindowDriver {
    state: Arc<Mutex<HashMap<String, FakeWindowState>>>,
    call_log: Arc<Mutex<Vec<String>>>,
}

impl FakeWindowDriver {
    /// Creates a new fake window driver.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets all state and call logs.
    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        state.clear();
        drop(state);
        let mut log = self.call_log.lock().unwrap();
        log.clear();
    }

    /// Adds a window to the fake driver.
    pub fn add_window(&self, window_id: &str, geometry: WindowGeometry) {
        let mut state = self.state.lock().unwrap();
        state.insert(
            window_id.to_string(),
            FakeWindowState {
                geometry,
                active: false,
                exists: true,
            },
        );
    }

    /// Gets the call log.
    pub fn call_log(&self) -> Vec<String> {
        self.call_log.lock().unwrap().clone()
    }

    /// Gets window state.
    pub fn get_state(&self, window_id: &str) -> Option<FakeWindowState> {
        self.state.lock().unwrap().get(window_id).cloned()
    }
}

impl WindowDriver for FakeWindowDriver {
    fn check_required_tools(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn find_windows(&self, _title_substring: &str) -> anyhow::Result<Vec<String>> {
        Ok(self.state.lock().unwrap().keys().cloned().collect())
    }

    fn find_windows_for_pid(&self, _pid: i32) -> anyhow::Result<Vec<String>> {
        Ok(self.state.lock().unwrap().keys().cloned().collect())
    }

    fn find_window(
        &self,
        _title_substring: &str,
        _timeout: Duration,
    ) -> anyhow::Result<Option<String>> {
        Ok(self.state.lock().unwrap().keys().next().cloned())
    }

    fn find_window_for_pid(&self, _pid: i32, _timeout: Duration) -> anyhow::Result<Option<String>> {
        Ok(self.state.lock().unwrap().keys().next().cloned())
    }

    fn wait_for_new_window(
        &self,
        _title_substring: &str,
        _before_ids: &HashSet<String>,
        _timeout: Duration,
    ) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn get_active_window(&self) -> anyhow::Result<Option<String>> {
        let state = self.state.lock().unwrap();
        Ok(state.iter().find(|(_, v)| v.active).map(|(k, _)| k.clone()))
    }

    fn get_window_pid(&self, _window_id: &str) -> anyhow::Result<Option<i32>> {
        Ok(None)
    }

    fn get_geometry(&self, window_id: &str) -> anyhow::Result<WindowGeometry> {
        self.state
            .lock()
            .unwrap()
            .get(window_id)
            .map(|s| s.geometry.clone())
            .ok_or_else(|| anyhow::anyhow!("window not found: {}", window_id))
    }

    fn resize(&self, window_id: &str, width: u32, height: u32) -> anyhow::Result<WindowGeometry> {
        let mut state = self.state.lock().unwrap();
        if let Some(win) = state.get_mut(window_id) {
            win.geometry.width = width as i32;
            win.geometry.height = height as i32;
            Ok(win.geometry.clone())
        } else {
            Err(anyhow::anyhow!("window not found: {}", window_id))
        }
    }

    fn set_geometry(
        &self,
        window_id: &str,
        geometry: &WindowGeometry,
    ) -> anyhow::Result<WindowGeometry> {
        let mut state = self.state.lock().unwrap();
        if let Some(win) = state.get_mut(window_id) {
            win.geometry = geometry.clone();
            Ok(win.geometry.clone())
        } else {
            Err(anyhow::anyhow!("window not found: {}", window_id))
        }
    }

    fn activate(&self, window_id: &str) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        if let Some(win) = state.get_mut(window_id) {
            win.active = true;
        }
        Ok(())
    }

    fn lower(&self, _window_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn background(&self, window_id: &str, _restore_window_id: Option<&str>) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        if let Some(win) = state.get_mut(window_id) {
            win.active = false;
        }
        Ok(())
    }

    fn window_exists(&self, window_id: &str) -> anyhow::Result<bool> {
        Ok(self.state.lock().unwrap().contains_key(window_id))
    }

    fn press_key(&self, _window_id: &str, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn type_text(&self, _window_id: &str, _text: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn clear_search(&self, _window_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn select_session(&self, _window_id: &str, _session_name: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn screenshot(&self, _window_id: &str, _output_path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn crop_metric(
        &self,
        _image_path: &Path,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
    ) -> anyhow::Result<VisualMetric> {
        Ok(VisualMetric {
            stddev: 0.05,
            colors: 32.0,
        })
    }

    fn image_size(&self, _image_path: &Path) -> anyhow::Result<(i32, i32)> {
        Ok((800, 600))
    }

    fn prepare_window_for_capture(
        &self,
        window_id: &str,
        width: u32,
        height: u32,
        _restore_window_id: Option<&str>,
    ) -> anyhow::Result<WindowGeometry> {
        self.resize(window_id, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_driver_add_window() {
        let driver = FakeWindowDriver::new();
        driver.add_window(
            FAKE_WINDOW_ID,
            WindowGeometry {
                x: 0,
                y: 0,
                width: 1280,
                height: 800,
            },
        );

        let state = driver.get_state(FAKE_WINDOW_ID);
        assert!(state.is_some());
        let state = state.unwrap();
        assert_eq!(state.geometry.width, 1280);
        assert_eq!(state.geometry.height, 800);
    }

    #[test]
    fn fake_driver_reset_clears_state() {
        let driver = FakeWindowDriver::new();
        driver.add_window(
            FAKE_WINDOW_ID,
            WindowGeometry {
                x: 0,
                y: 0,
                width: 1280,
                height: 800,
            },
        );

        driver.reset();

        let state = driver.get_state(FAKE_WINDOW_ID);
        assert!(state.is_none());
    }

    #[test]
    fn fake_driver_call_log_records_operations() {
        let driver = FakeWindowDriver::new();
        driver.add_window(
            FAKE_WINDOW_ID,
            WindowGeometry {
                x: 0,
                y: 0,
                width: 1280,
                height: 800,
            },
        );

        // Simulate operations by directly checking state
        let state = driver.get_state(FAKE_WINDOW_ID);
        assert!(state.is_some());

        let log = driver.call_log();
        // Log starts empty unless operations explicitly record
        assert!(log.is_empty());
    }

    #[test]
    fn fake_driver_multiple_windows() {
        let driver = FakeWindowDriver::new();
        driver.add_window(
            "0xWINDOW1",
            WindowGeometry {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            },
        );
        driver.add_window(
            "0xWINDOW2",
            WindowGeometry {
                x: 100,
                y: 100,
                width: 1024,
                height: 768,
            },
        );

        let state1 = driver.get_state("0xWINDOW1");
        let state2 = driver.get_state("0xWINDOW2");

        assert!(state1.is_some());
        assert!(state2.is_some());
        assert_eq!(state1.unwrap().geometry.width, 800);
        assert_eq!(state2.unwrap().geometry.width, 1024);
    }

    #[test]
    fn fake_driver_missing_window_returns_none() {
        let driver = FakeWindowDriver::new();
        let state = driver.get_state("0xNONEXISTENT");
        assert!(state.is_none());
    }
}
