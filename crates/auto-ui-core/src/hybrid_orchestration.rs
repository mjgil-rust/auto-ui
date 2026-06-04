//! Generic hybrid orchestration combining startup-driven and interactive-window modes.
//!
//! This module provides orchestration for scenarios that need both:
//! 1. Startup-driven launch (app drives itself after startup)
//! 2. Interactive window management (resize, capture, etc.)

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;

use crate::{
    InteractiveOrchestrationConfig, InteractiveWindowOrchestrator, LaunchedRun, PreparedRun,
    StartupOrchestrationConfig, StartupOrchestrator, WindowSelector, WindowState,
};

/// Configuration for hybrid orchestration.
///
/// Combines startup-driven and interactive window configuration.
#[derive(Clone, Debug, Default)]
pub struct HybridOrchestrationConfig {
    /// Startup orchestration configuration.
    pub startup_config: StartupOrchestrationConfig,
    /// Interactive window configuration.
    pub interactive_config: InteractiveOrchestrationConfig,
}

impl HybridOrchestrationConfig {
    /// Creates a new config with startup-driven settings.
    pub fn with_startup_config(mut self, config: StartupOrchestrationConfig) -> Self {
        self.startup_config = config;
        self
    }

    /// Creates a new config with interactive window settings.
    pub fn with_interactive_config(mut self, config: InteractiveOrchestrationConfig) -> Self {
        self.interactive_config = config;
        self
    }

    /// Sets the startup timeout.
    pub fn with_startup_timeout(mut self, timeout: Duration) -> Self {
        self.startup_config = self.startup_config.with_timeout(timeout);
        self
    }

    /// Sets the initial window geometry.
    pub fn with_geometry(mut self, width: u32, height: u32) -> Self {
        self.interactive_config = self.interactive_config.with_geometry(width, height);
        self
    }

    /// Sets the window selector for post-launch window attachment.
    pub fn with_window_selector(mut self, selector: WindowSelector) -> Self {
        self.interactive_config = self.interactive_config.with_window_selector(selector);
        self
    }

    /// Sets whether to keep window in front during manipulation.
    pub fn with_keep_front(mut self, keep_front: bool) -> Self {
        self.interactive_config = self.interactive_config.with_keep_front(keep_front);
        self
    }
}

/// Orchestrator for hybrid execution modes.
///
/// Combines startup-driven process launch with interactive window management.
/// Use for scenarios like rust-chatbot where we:
/// 1. Launch directly into a target session (startup-driven)
/// 2. Resize and capture the window (interactive window)
/// 3. Let the app continue running while we do capture operations
#[derive(Default)]
pub struct HybridOrchestrator {
    config: HybridOrchestrationConfig,
}

impl HybridOrchestrator {
    /// Creates a new orchestrator with the given configuration.
    pub fn new(config: HybridOrchestrationConfig) -> Self {
        Self { config }
    }

    /// Creates a new orchestrator with default configuration.
    pub fn default_config() -> Self {
        Self::default()
    }

    /// Runs a hybrid scenario with launch-then-resize/capture flow.
    ///
    /// This orchestrates:
    /// 1. Launch the process via StartupOrchestrator
    /// 2. Wait for process startup
    /// 3. Attach to the window via InteractiveWindowOrchestrator
    /// 4. Apply initial geometry if configured
    /// 5. Optionally activate (keep_front) or lower
    /// 6. Return state for potential capture/restoration
    pub fn run_hybrid_scenario(
        &self,
        prepared: &PreparedRun,
    ) -> Result<(LaunchedRun, Option<WindowState>)> {
        // Phase 1: Startup-driven launch
        let startup = StartupOrchestrator::new(self.config.startup_config.clone());
        let launched = startup.launch_and_wait(prepared)?;

        // Phase 2: Interactive window management
        // If we have a window_id, apply interactive orchestration
        let window_state = if launched.window_id.is_some() {
            let interactive =
                InteractiveWindowOrchestrator::new(self.config.interactive_config.clone());
            Some(interactive.run_interactive_scenario(&launched)?)
        } else {
            None
        };

        Ok((launched, window_state))
    }

    /// Runs a hybrid scenario that captures after launch.
    ///
    /// Variant that specifically handles:
    /// 1. Launch via startup orchestrator
    /// 2. Wait for window to appear
    /// 3. Attach and resize
    /// 4. Capture screenshot
    /// 5. Restore window state
    pub fn run_hybrid_with_capture(
        &self,
        prepared: &PreparedRun,
        output_path: &PathBuf,
    ) -> Result<(LaunchedRun, Option<WindowState>)> {
        // Launch first
        let startup = StartupOrchestrator::new(self.config.startup_config.clone());
        let launched = startup.launch_and_wait(prepared)?;

        // If we have a window_id, do interactive capture
        if let Some(ref window_id) = launched.window_id {
            let interactive =
                InteractiveWindowOrchestrator::new(self.config.interactive_config.clone());

            // Attach to get initial state
            let state = interactive.attach_to_window(window_id)?;

            // Apply geometry if configured
            if let Some((width, height)) = self.config.interactive_config.initial_geometry {
                interactive.resize_window(window_id, width, height)?;
            }

            // Activate if keep_front
            if self.config.interactive_config.keep_front {
                interactive.activate_window(window_id)?;
            }

            // Capture screenshot
            interactive.screenshot_window(window_id, output_path)?;

            // Lower if not keep_front
            if !self.config.interactive_config.keep_front {
                interactive.lower_window(window_id)?;
            }

            // Restore state
            interactive.restore_window_state(window_id, &state)?;

            return Ok((launched, Some(state)));
        }

        Ok((launched, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandSpec;

    #[test]
    fn hybrid_config_default() {
        let config = HybridOrchestrationConfig::default();
        assert!(matches!(
            config.interactive_config.window_selector,
            WindowSelector::Active
        ));
        assert!(!config.interactive_config.keep_front);
    }

    #[test]
    fn hybrid_config_builder() {
        let config = HybridOrchestrationConfig::default()
            .with_geometry(1024, 768)
            .with_keep_front(true)
            .with_window_selector(WindowSelector::Pid { value: 1234 });

        assert_eq!(
            config.interactive_config.initial_geometry,
            Some((1024, 768))
        );
        assert!(config.interactive_config.keep_front);
        assert!(matches!(
            config.interactive_config.window_selector,
            WindowSelector::Pid { value: 1234 }
        ));
    }

    #[test]
    fn hybrid_orchestrator_spawns_and_returns_launched() {
        let prepared = PreparedRun {
            strategy: crate::LaunchStrategy::AutonomousProcess {
                command: CommandSpec::new("/bin/echo").arg("hybrid test"),
                background: false,
            },
            ..Default::default()
        };

        let config = HybridOrchestrationConfig::default();
        let orchestrator = HybridOrchestrator::new(config);

        // This will succeed since we're launching a simple echo command
        let result = orchestrator.run_hybrid_scenario(&prepared);
        assert!(result.is_ok());
        let (launched, window_state) = result.unwrap();
        assert!(launched.pid.is_some());
        // No window state since the launched process has no window
        assert!(window_state.is_none());
    }

    #[test]
    fn hybrid_config_with_startup_timeout() {
        let config =
            HybridOrchestrationConfig::default().with_startup_timeout(Duration::from_secs(60));

        assert_eq!(config.startup_config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn hybrid_config_combines_startup_and_interactive() {
        let startup_cfg =
            StartupOrchestrationConfig::default().with_timeout(Duration::from_secs(120));

        let interactive_cfg = InteractiveOrchestrationConfig::default().with_geometry(800, 600);

        let config = HybridOrchestrationConfig::default()
            .with_startup_config(startup_cfg)
            .with_interactive_config(interactive_cfg);

        assert_eq!(config.startup_config.timeout, Duration::from_secs(120));
        assert_eq!(config.interactive_config.initial_geometry, Some((800, 600)));
    }

    #[test]
    fn hybrid_orchestrator_with_existing_window_strategy() {
        // When prepared has ExistingWindow strategy, startup orchestrator should fail
        // but in this case we test that the architecture handles it properly
        let prepared = PreparedRun {
            strategy: crate::LaunchStrategy::ExistingWindow {
                selector: WindowSelector::Active,
            },
            ..Default::default()
        };

        let config = HybridOrchestrationConfig::default();
        let orchestrator = HybridOrchestrator::new(config);

        // This should fail because ExistingWindow is not supported by StartupOrchestrator
        let result = orchestrator.run_hybrid_scenario(&prepared);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("ExistingWindow"));
    }

    #[test]
    fn hybrid_config_debug_format() {
        let config = HybridOrchestrationConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("HybridOrchestrationConfig"));
    }
}
