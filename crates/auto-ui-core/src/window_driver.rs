//! Pluggable window automation trait and shared types.
//!
//! This module defines the `WindowDriver` trait that abstracts window
//! management operations across platforms (X11, macOS, etc.).

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Window geometry information.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Visual metric computed from a cropped image region.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VisualMetric {
    pub stddev: f64,
    pub colors: f64,
}

/// Trait for pluggable window automation backends.
///
/// Implementers provide real window management via platform-specific APIs
/// (X11 tools, macOS Accessibility, etc.) or fake implementations for testing.
pub trait WindowDriver: Send + Sync {
    /// Validate that required OS-level dependencies are present.
    fn check_required_tools(&self) -> Result<()>;

    /// Find windows matching a title substring.
    fn find_windows(&self, title_substring: &str) -> Result<Vec<String>>;

    /// Find windows for a specific process ID.
    fn find_windows_for_pid(&self, pid: i32) -> Result<Vec<String>>;

    /// Find a single window by title substring, waiting up to `timeout`.
    fn find_window(&self, title_substring: &str, timeout: Duration) -> Result<Option<String>>;

    /// Find a single window by PID, waiting up to `timeout`.
    fn find_window_for_pid(&self, pid: i32, timeout: Duration) -> Result<Option<String>>;

    /// Wait for a new window to appear that wasn't in `before_ids`.
    fn wait_for_new_window(
        &self,
        title_substring: &str,
        before_ids: &HashSet<String>,
        timeout: Duration,
    ) -> Result<Option<String>>;

    /// Get the currently active window ID.
    fn get_active_window(&self) -> Result<Option<String>>;

    /// Get the PID owning a window.
    fn get_window_pid(&self, window_id: &str) -> Result<Option<i32>>;

    /// Get the geometry of a window.
    fn get_geometry(&self, window_id: &str) -> Result<WindowGeometry>;

    /// Resize a window to specific dimensions.
    fn resize(&self, window_id: &str, width: u32, height: u32) -> Result<WindowGeometry>;

    /// Set both position and size of a window.
    fn set_geometry(&self, window_id: &str, geometry: &WindowGeometry) -> Result<WindowGeometry>;

    /// Activate (focus) a window.
    fn activate(&self, window_id: &str) -> Result<()>;

    /// Lower a window to the bottom.
    fn lower(&self, window_id: &str) -> Result<()>;

    /// Lower a window and optionally restore the previous active window.
    fn background(&self, window_id: &str, restore_window_id: Option<&str>) -> Result<()>;

    /// Check if a window still exists.
    fn window_exists(&self, window_id: &str) -> Result<bool>;

    /// Press a key in a window.
    fn press_key(&self, window_id: &str, key: &str) -> Result<()>;

    /// Type text into a window.
    fn type_text(&self, window_id: &str, text: &str) -> Result<()>;

    /// Clear a search field (Ctrl+A, BackSpace).
    fn clear_search(&self, window_id: &str) -> Result<()>;

    /// Select a session by name (Ctrl+F, type, Return).
    fn select_session(&self, window_id: &str, session_name: &str) -> Result<()>;

    /// Capture a screenshot of a window.
    fn screenshot(&self, window_id: &str, output_path: &Path) -> Result<()>;

    /// Compute visual metrics for a cropped region of an image.
    fn crop_metric(
        &self,
        image_path: &Path,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<VisualMetric>;

    /// Get the dimensions of an image.
    fn image_size(&self, image_path: &Path) -> Result<(i32, i32)>;
}

/// Default heuristic for whether text is visible in a cropped region.
pub fn heuristic_text_visible(metric: &VisualMetric) -> bool {
    metric.stddev >= 0.01 || metric.colors >= 16.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_geometry_default() {
        let g = WindowGeometry::default();
        assert_eq!(g.x, 0);
        assert_eq!(g.y, 0);
        assert_eq!(g.width, 0);
        assert_eq!(g.height, 0);
    }

    #[test]
    fn visual_metric_default() {
        let m = VisualMetric::default();
        assert_eq!(m.stddev, 0.0);
        assert_eq!(m.colors, 0.0);
    }

    #[test]
    fn heuristic_text_visible_boundary() {
        assert!(!heuristic_text_visible(&VisualMetric {
            stddev: 0.0,
            colors: 0.0,
        }));
        assert!(!heuristic_text_visible(&VisualMetric {
            stddev: 0.005,
            colors: 8.0,
        }));
        assert!(heuristic_text_visible(&VisualMetric {
            stddev: 0.01,
            colors: 0.0,
        }));
        assert!(heuristic_text_visible(&VisualMetric {
            stddev: 0.0,
            colors: 16.0,
        }));
        assert!(heuristic_text_visible(&VisualMetric {
            stddev: 0.02,
            colors: 32.0,
        }));
    }

    #[test]
    fn trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn WindowDriver) {}
    }
}
