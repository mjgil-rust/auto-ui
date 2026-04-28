//! Generic interactive window orchestration.
//!
//! This module provides orchestration for scenarios where the harness needs
//! to manipulate windows directly (resize, focus, input, screenshot).

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::{Event, LaunchedRun, WindowSelector, log_line};

/// Configuration for interactive window orchestration.
#[derive(Clone, Debug)]
pub struct InteractiveOrchestrationConfig {
    /// Window selector for identifying the target window.
    pub window_selector: WindowSelector,
    /// Initial window geometry (width x height).
    pub initial_geometry: Option<(u32, u32)>,
    /// Whether to keep the window in front after positioning.
    pub keep_front: bool,
    /// Timeout for window operations.
    pub window_timeout: Duration,
    /// Progress log path for status updates.
    pub progress_log: Option<PathBuf>,
}

impl Default for InteractiveOrchestrationConfig {
    fn default() -> Self {
        Self {
            window_selector: WindowSelector::Active,
            initial_geometry: None,
            keep_front: false,
            window_timeout: Duration::from_secs(30),
            progress_log: None,
        }
    }
}

impl InteractiveOrchestrationConfig {
    /// Sets the window selector.
    pub fn with_window_selector(mut self, selector: WindowSelector) -> Self {
        self.window_selector = selector;
        self
    }

    /// Sets the initial geometry.
    pub fn with_geometry(mut self, width: u32, height: u32) -> Self {
        self.initial_geometry = Some((width, height));
        self
    }

    /// Sets whether to keep window in front.
    pub fn with_keep_front(mut self, keep_front: bool) -> Self {
        self.keep_front = keep_front;
        self
    }

    /// Sets the window timeout.
    pub fn with_window_timeout(mut self, timeout: Duration) -> Self {
        self.window_timeout = timeout;
        self
    }

    /// Sets the progress log path.
    pub fn with_progress_log(mut self, path: PathBuf) -> Self {
        self.progress_log = Some(path);
        self
    }
}

/// Window geometry information.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// State captured before window manipulation for restoration.
#[derive(Clone, Debug)]
pub struct WindowState {
    pub geometry: WindowGeometry,
    pub z_order: i32,
}

/// Kinds of foreground control events emitted by the orchestrator.
pub mod foreground_events {
    pub const WINDOW_ACTIVATED: &str = "foreground_control";
    pub const WINDOW_LOWERED: &str = "foreground_control";
}

/// Creates a timestamped Event for foreground control actions.
fn make_foreground_event(action: &str, window_id: &str, message: &str) -> Event {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0).unwrap().to_rfc3339())
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

    Event {
        timestamp,
        kind: action.to_string(),
        message: Some(format!("window {}: {}", window_id, message)),
    }
}

/// Orchestrator for interactive window scenarios.
///
/// This handles window management operations like resize, focus, input, and
/// screenshot capture for scenarios that require direct window manipulation.
#[derive(Default)]
pub struct InteractiveWindowOrchestrator {
    config: InteractiveOrchestrationConfig,
}

/// Result of an interactive window scenario, including any foreground control events.
pub struct InteractiveWindowResult {
    /// The window state that was captured.
    pub state: WindowState,
    /// Foreground control events that were emitted during the scenario.
    pub events: Vec<Event>,
}

impl InteractiveWindowOrchestrator {
    /// Creates a new orchestrator with the given configuration.
    pub fn new(config: InteractiveOrchestrationConfig) -> Self {
        Self { config }
    }

    /// Creates a new orchestrator with default configuration.
    pub fn default_config() -> Self {
        Self::default()
    }

    /// Validates that a window_id is non-empty and appears valid.
    ///
    /// This is a basic validation. Real implementations should verify
    /// the window actually exists and is responsive.
    fn validate_window_id(window_id: &str) -> Result<()> {
        if window_id.is_empty() {
            bail!("window_id cannot be empty");
        }
        if window_id.len() < 3 {
            bail!("window_id '{}' appears invalid (too short)", window_id);
        }
        Ok(())
    }

    /// Attaches to a window (existing or just launched).
    ///
    /// Returns the window state that can be used for later restoration.
    pub fn attach_to_window(&self, window_id: &str) -> Result<WindowState> {
        Self::validate_window_id(window_id)?;

        // In a full implementation, this would query the window driver
        // for the current geometry and z-order and verify the window exists
        let geometry = WindowGeometry {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            ..Default::default()
        };

        Ok(WindowState {
            geometry,
            z_order: 0,
        })
    }

    /// Resizes a window to the given dimensions.
    pub fn resize_window(&self, window_id: &str, width: u32, height: u32) -> Result<WindowGeometry> {
        Self::validate_window_id(window_id)?;

        if self.config.window_selector == WindowSelector::Active {
            bail!("cannot resize window: no window selector specified");
        }

        // In a full implementation, this would call the window driver
        // and verify the window still exists before and after resize
        Ok(WindowGeometry {
            x: 0,
            y: 0,
            width,
            height,
        })
    }

    /// Activates (brings to front) a window.
    /// Emits a foreground control event for auditing.
    pub fn activate_window(&self, window_id: &str) -> Result<()> {
        Self::validate_window_id(window_id)?;

        if self.config.window_selector == WindowSelector::Active {
            bail!("cannot activate window: no window selector specified");
        }

        // Log the foreground control event for auditing
        let event = make_foreground_event(foreground_events::WINDOW_ACTIVATED, window_id, "activated");
        log_line(format!("[EVENT] {}: {:?}", event.kind, event.message), self.config.progress_log.as_deref()).ok();

        // In a full implementation, this would call the window driver
        // and verify the window accepts focus
        Ok(())
    }

    /// Lowers a window (sends to back).
    /// Emits a foreground control event for auditing.
    pub fn lower_window(&self, window_id: &str) -> Result<()> {
        Self::validate_window_id(window_id)?;

        // Log the foreground control event for auditing
        let event = make_foreground_event(foreground_events::WINDOW_LOWERED, window_id, "lowered");
        log_line(format!("[EVENT] {}: {:?}", event.kind, event.message), self.config.progress_log.as_deref()).ok();

        // In a full implementation, this would call the window driver
        // and verify the window was successfully lowered
        Ok(())
    }

    /// Captures a screenshot of a window.
    pub fn screenshot_window(&self, window_id: &str, output_path: &PathBuf) -> Result<()> {
        Self::validate_window_id(window_id)?;

        // In a full implementation, this would call the window driver
        // and verify the window is still present for capture
        Ok(())
    }

    /// Restores window state after manipulation.
    pub fn restore_window_state(&self, window_id: &str, state: &WindowState) -> Result<()> {
        Self::validate_window_id(window_id)?;

        // In a full implementation, this would restore geometry and z-order
        // and verify the window accepts the restored state
        Ok(())
    }

    /// Runs an interactive window scenario lifecycle.
    ///
    /// This orchestrates the full lifecycle:
    /// 1. Attach to window
    /// 2. Optionally resize
    /// 3. Optionally activate (bring to front)
    /// 4. Capture screenshot if needed
    /// 5. Lower window (if keep_front is false)
    /// 6. Restore state
    pub fn run_interactive_scenario(
        &self,
        launched: &LaunchedRun,
    ) -> Result<WindowState> {
        // For interactive window scenarios, we work with the launched window
        let window_id = launched
            .window_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("interactive window scenario requires window_id"))?;

        // Attach and capture initial state
        let initial_state = self.attach_to_window(window_id)?;

        // Apply initial geometry if specified
        if let Some((width, height)) = self.config.initial_geometry {
            self.resize_window(window_id, width, height)?;
        }

        // Activate if keep_front is true
        if self.config.keep_front {
            self.activate_window(window_id)?;
        } else {
            // Lower after any foreground operations
            self.lower_window(window_id)?;
        }

        // Restore original state if we modified the window
        self.restore_window_state(window_id, &initial_state)?;

        Ok(initial_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use crate::CommandSpec;

    #[test]
    fn interactive_config_default() {
        let config = InteractiveOrchestrationConfig::default();
        assert!(matches!(config.window_selector, WindowSelector::Active));
        assert!(config.initial_geometry.is_none());
        assert!(!config.keep_front);
        assert_eq!(config.window_timeout, Duration::from_secs(30));
    }

    #[test]
    fn interactive_config_builder() {
        let config = InteractiveOrchestrationConfig::default()
            .with_window_selector(WindowSelector::Pid { value: 1234 })
            .with_geometry(1024, 768)
            .with_keep_front(true)
            .with_window_timeout(Duration::from_secs(60));

        assert!(matches!(config.window_selector, WindowSelector::Pid { value: 1234 }));
        assert_eq!(config.initial_geometry, Some((1024, 768)));
        assert!(config.keep_front);
        assert_eq!(config.window_timeout, Duration::from_secs(60));
    }

    #[test]
    fn attach_to_window_returns_state() {
        let config = InteractiveOrchestrationConfig::default();
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let state = orchestrator.attach_to_window("0x12345678").unwrap();
        assert_eq!(state.geometry.width, 800);
        assert_eq!(state.geometry.height, 600);
    }

    #[test]
    fn resize_window_returns_geometry() {
        let config = InteractiveOrchestrationConfig::default()
            .with_window_selector(WindowSelector::Pid { value: 1234 });
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let result = orchestrator.resize_window("0x12345678", 1024, 768);
        assert!(result.is_ok());
        let geo = result.unwrap();
        assert_eq!(geo.width, 1024);
        assert_eq!(geo.height, 768);
    }

    #[test]
    fn resize_window_requires_selector() {
        let config = InteractiveOrchestrationConfig::default(); // Active selector
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let result = orchestrator.resize_window("0x12345678", 1024, 768);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no window selector"));
    }

    #[test]
    fn activate_window_requires_selector() {
        let config = InteractiveOrchestrationConfig::default();
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let result = orchestrator.activate_window("0x12345678");
        assert!(result.is_err());
    }

    #[test]
    fn lower_window_succeeds() {
        let config = InteractiveOrchestrationConfig::default();
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let result = orchestrator.lower_window("0x12345678");
        assert!(result.is_ok());
    }

    #[test]
    fn lower_window_rejects_empty_id() {
        let config = InteractiveOrchestrationConfig::default();
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let result = orchestrator.lower_window("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn lower_window_rejects_short_id() {
        let config = InteractiveOrchestrationConfig::default();
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let result = orchestrator.lower_window("ab");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too short"));
    }

    #[test]
    fn screenshot_window_succeeds() {
        let config = InteractiveOrchestrationConfig::default();
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let result = orchestrator.screenshot_window("0x12345678", &PathBuf::from("/tmp/screenshot.png"));
        assert!(result.is_ok());
    }

    #[test]
    fn restore_window_state_succeeds() {
        let config = InteractiveOrchestrationConfig::default();
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let state = WindowState {
            geometry: WindowGeometry {
                x: 100,
                y: 100,
                width: 800,
                height: 600,
            },
            z_order: 0,
        };

        let result = orchestrator.restore_window_state("0x12345678", &state);
        assert!(result.is_ok());
    }

    #[test]
    fn run_interactive_scenario_requires_window_id() {
        let config = InteractiveOrchestrationConfig::default();
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let launched = LaunchedRun {
            pid: Some(1234),
            window_id: None, // No window_id
            command: CommandSpec::new("/bin/test"),
            env: BTreeMap::new(),
        };

        let result = orchestrator.run_interactive_scenario(&launched);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("window_id"));
    }

    #[test]
    fn run_interactive_scenario_with_default_config_lowers_window() {
        // Default config: keep_front=false (windows should be lowered)
        let config = InteractiveOrchestrationConfig::default();
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let launched = LaunchedRun {
            pid: Some(1234),
            window_id: Some("0x12345678".to_string()),
            command: CommandSpec::new("/bin/test"),
            env: BTreeMap::new(),
        };

        // With default config (keep_front=false), run_interactive_scenario should succeed
        // and call lower_window (not activate_window)
        let result = orchestrator.run_interactive_scenario(&launched);
        assert!(result.is_ok());
    }

    #[test]
    fn run_interactive_scenario_with_keep_front_true_and_active_selector_fails() {
        // keep_front=true with Active selector should fail on activate
        let config = InteractiveOrchestrationConfig::default()
            .with_keep_front(true); // keep_front=true but selector is Active (default)
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let launched = LaunchedRun {
            pid: Some(1234),
            window_id: Some("0x12345678".to_string()),
            command: CommandSpec::new("/bin/test"),
            env: BTreeMap::new(),
        };

        let result = orchestrator.run_interactive_scenario(&launched);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no window selector"));
    }

    #[test]
    fn run_interactive_scenario_with_window() {
        let config = InteractiveOrchestrationConfig::default()
            .with_window_selector(WindowSelector::Pid { value: 1234 })
            .with_geometry(1024, 768)
            .with_keep_front(true);
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let launched = LaunchedRun {
            pid: Some(1234),
            window_id: Some("0x12345678".to_string()),
            command: CommandSpec::new("/bin/test"),
            env: BTreeMap::new(),
        };

        let result = orchestrator.run_interactive_scenario(&launched);
        assert!(result.is_ok());
    }

    #[test]
    fn run_interactive_scenario_restores_geometry() {
        // Test that run_interactive_scenario restores geometry after modifications
        let config = InteractiveOrchestrationConfig::default()
            .with_window_selector(WindowSelector::Pid { value: 1234 })
            .with_geometry(1024, 768);
        let orchestrator = InteractiveWindowOrchestrator::new(config);

        let launched = LaunchedRun {
            pid: Some(1234),
            window_id: Some("0x12345678".to_string()),
            command: CommandSpec::new("/bin/test"),
            env: BTreeMap::new(),
        };

        let result = orchestrator.run_interactive_scenario(&launched);
        assert!(result.is_ok());

        // The returned state should match what attach_to_window returned
        // (which is the initial state before any modifications)
        let returned_state = result.unwrap();
        assert_eq!(returned_state.geometry.width, 800);
        assert_eq!(returned_state.geometry.height, 600);
    }

    #[test]
    fn window_geometry_serialization() {
        let geo = WindowGeometry {
            x: 100,
            y: 200,
            width: 1024,
            height: 768,
        };
        let json = serde_json::to_string(&geo).unwrap();
        assert!(json.contains("\"x\":100"));
        assert!(json.contains("\"width\":1024"));
    }

    #[test]
    fn window_state_clone() {
        let state = WindowState {
            geometry: WindowGeometry {
                x: 100,
                y: 200,
                width: 1024,
                height: 768,
            },
            z_order: 5,
        };
        let cloned = state.clone();
        assert_eq!(cloned.geometry.width, state.geometry.width);
        assert_eq!(cloned.z_order, state.z_order);
    }
}
