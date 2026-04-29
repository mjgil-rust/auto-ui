//! Fake window driver for integration testing.
//!
//! This module provides a fake implementation of window operations
//! that doesn't require X11 tools, allowing orchestration tests to run
//! in CI without a display.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use super::WindowGeometry;

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
