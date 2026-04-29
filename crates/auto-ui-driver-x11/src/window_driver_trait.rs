//! WindowDriver trait for abstracting window operations.
//!
//! This trait provides a common interface for window management operations
//! that can be implemented by real X11 drivers or fake drivers for testing.

use crate::WindowGeometry;
use anyhow::Result;
use std::path::PathBuf;

/// Result type for window operations that can return None when no window is found.
pub type WindowResult<T> = Result<T>;

/// Trait for window driver operations.
///
/// Implementers must provide real window management via X11 tools (xdotool, wmctrl, etc.)
/// or fake implementations for testing.
pub trait WindowDriver: Send + Sync {
    /// Find windows matching a title substring.
    fn find_windows(&self, title: &str) -> Result<Vec<String>>;

    /// Find windows for a specific process ID.
    fn find_windows_for_pid(&self, pid: i32) -> Result<Vec<String>>;

    /// Get the geometry of a window.
    fn get_geometry(&self, window_id: &str) -> Result<WindowGeometry>;

    /// Check if a window exists.
    fn window_exists(&self, window_id: &str) -> Result<bool>;

    /// Activate (focus) a window.
    fn activate(&self, window_id: &str) -> Result<()>;

    /// Lower a window to the bottom.
    fn lower(&self, window_id: &str) -> Result<()>;

    /// Resize a window to specific dimensions.
    fn resize(&self, window_id: &str, width: u32, height: u32) -> Result<WindowGeometry>;

    /// Capture a screenshot of a window.
    fn screenshot(&self, window_id: &str, output_path: &PathBuf) -> Result<()>;

    /// Press a key in a window.
    fn press_key(&self, window_id: &str, key: &str) -> Result<()>;

    /// Get the currently active window ID.
    fn get_active_window(&self) -> Result<Option<String>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_result_type_alias() {
        // WindowResult should be usable as Result<T>
        let result: WindowResult<i32> = Ok(42);
        assert_eq!(result.unwrap(), 42);
    }
}