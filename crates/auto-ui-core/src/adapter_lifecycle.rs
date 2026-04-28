//! Centralized adapter lifecycle management.
//!
//! This module provides utilities for ensuring proper cleanup of adapter resources
//! even when errors occur during the launch process.

use anyhow::Result;

use crate::{AdapterContext, CollectedData, LaunchedRun, TargetAdapter};

/// Result of an adapter lifecycle operation.
///
/// This type captures both successful results and cleanup errors,
/// ensuring that the original error is preserved even if cleanup fails.
#[derive(Debug)]
pub struct LifecycleResult<T> {
    /// The result of the operation.
    pub result: Result<T>,
    /// Whether cleanup (stop) was called.
    pub cleanup_called: bool,
    /// Error that occurred during cleanup, if any.
    pub cleanup_error: Option<String>,
}

impl<T> LifecycleResult<T> {
    /// Creates a successful lifecycle result.
    pub fn success(value: T) -> Self {
        Self {
            result: Ok(value),
            cleanup_called: false,
            cleanup_error: None,
        }
    }

    /// Creates a failed lifecycle result.
    pub fn failure(error: anyhow::Error) -> Self {
        Self {
            result: Err(error),
            cleanup_called: false,
            cleanup_error: None,
        }
    }

    /// Returns whether the operation succeeded.
    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }

    /// Returns the inner value if successful.
    pub fn ok(self) -> Option<T> {
        self.result.ok()
    }

    /// Returns the inner error if failed.
    pub fn err(self) -> Option<anyhow::Error> {
        self.result.err()
    }

    /// Returns the cleanup error if cleanup failed.
    pub fn cleanup_err(&self) -> Option<&str> {
        self.cleanup_error.as_deref()
    }
}

/// Runs the full adapter lifecycle (prepare -> launch -> collect -> stop).
///
/// This function ensures that `stop` is always called on the adapter,
/// even if `launch` or `collect` fails. If cleanup fails, the original
/// error is preserved and the cleanup error is attached to the result.
pub fn run_lifecycle<A: TargetAdapter>(
    adapter: &A,
    ctx: &AdapterContext,
    spec: &crate::ScenarioSpec,
) -> LifecycleResult<(LaunchedRun, CollectedData)> {
    // Phase 1: Prepare
    let prepared = match adapter.prepare(ctx, spec) {
        Ok(p) => p,
        Err(e) => return LifecycleResult::failure(e),
    };

    // Phase 2: Launch with cleanup guarantee
    let launched = match adapter.launch(ctx, &prepared) {
        Ok(l) => l,
        Err(e) => {
            // Launch failed - still try to clean up
            let cleanup_result = run_cleanup(adapter, ctx, None);
            return LifecycleResult {
                result: Err(e),
                cleanup_called: cleanup_result.called,
                cleanup_error: cleanup_result.error,
            };
        }
    };

    // Phase 3: Collect
    let collected = match adapter.collect(ctx, &launched) {
        Ok(c) => c,
        Err(e) => {
            // Collect failed - clean up and preserve original error
            let cleanup_result = run_cleanup(adapter, ctx, Some(&launched));
            return LifecycleResult {
                result: Err(e),
                cleanup_called: cleanup_result.called,
                cleanup_error: cleanup_result.error,
            };
        }
    };

    // Phase 4: Stop (normal cleanup)
    let cleanup_result = run_cleanup(adapter, ctx, Some(&launched));

    LifecycleResult {
        result: Ok((launched, collected)),
        cleanup_called: cleanup_result.called,
        cleanup_error: cleanup_result.error,
    }
}

/// Helper to run cleanup, returning information about whether it succeeded.
fn run_cleanup<A: TargetAdapter>(
    adapter: &A,
    ctx: &AdapterContext,
    launched: Option<&LaunchedRun>,
) -> CleanupResult {
    if let Some(run) = launched {
        match adapter.stop(ctx, run) {
            Ok(()) => CleanupResult {
                called: true,
                error: None,
            },
            Err(e) => CleanupResult {
                called: true,
                error: Some(e.to_string()),
            },
        }
    } else {
        CleanupResult {
            called: false,
            error: None,
        }
    }
}

#[derive(Debug)]
struct CleanupResult {
    called: bool,
    error: Option<String>,
}

/// Runs the full adapter lifecycle and returns only the collected data.
///
/// This is a convenience function that ignores the LaunchedRun handle
/// and returns only the CollectedData on success.
pub fn run_lifecycle_collect<A: TargetAdapter>(
    adapter: &A,
    ctx: &AdapterContext,
    spec: &crate::ScenarioSpec,
) -> LifecycleResult<CollectedData> {
    let result = run_lifecycle(adapter, ctx, spec);
    match result.result {
        Ok((_, collected)) => LifecycleResult {
            result: Ok(collected),
            cleanup_called: result.cleanup_called,
            cleanup_error: result.cleanup_error,
        },
        Err(e) => LifecycleResult {
            result: Err(e),
            cleanup_called: result.cleanup_called,
            cleanup_error: result.cleanup_error,
        },
    }
}

/// Error type that preserves both the original error and any cleanup error.
#[derive(Debug)]
pub struct LifecycleError {
    /// The original error that caused the lifecycle to fail.
    pub original: String,
    /// Error that occurred during cleanup, if any.
    pub cleanup: Option<String>,
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lifecycle error: {}", self.original)?;
        if let Some(ref cleanup) = self.cleanup {
            write!(f, " (cleanup failed: {})", cleanup)?;
        }
        Ok(())
    }
}

impl std::error::Error for LifecycleError {}

/// Trait for types that can report their lifecycle status.
pub trait LifecycleReporter {
    /// Reports the lifecycle result in a structured way.
    fn report_lifecycle(&self, result: &LifecycleResult<CollectedData>) -> String;
}

impl<T> LifecycleReporter for LifecycleResult<T> {
    fn report_lifecycle(&self, result: &LifecycleResult<CollectedData>) -> String {
        if result.is_success() {
            format!(
                "lifecycle completed successfully (cleanup: {})",
                if result.cleanup_called { "called" } else { "not needed" }
            )
        } else {
            let err = result.result.as_ref().unwrap_err();
            format!(
                "lifecycle failed: {} (cleanup: {}, cleanup_error: {:?})",
                err,
                if result.cleanup_called { "called" } else { "not called" },
                result.cleanup_error
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdapterContext, ScenarioRef, ScenarioSpec, PreparedRun, LaunchedRun, CollectedData, CommandSpec};
    use anyhow::bail;
    use std::path::PathBuf;

    struct FailingAdapter {
        fail_on: &'static str,
    }

    impl FailingAdapter {
        fn new(fail_on: &'static str) -> Self {
            Self { fail_on }
        }
    }

    impl TargetAdapter for FailingAdapter {
        fn id(&self) -> &'static str {
            "failing_adapter"
        }

        fn discover_scenarios(&self) -> Vec<ScenarioRef> {
            vec![ScenarioRef { name: "test".to_string(), description: None }]
        }

        fn prepare(
            &self,
            _ctx: &AdapterContext,
            _spec: &ScenarioSpec,
        ) -> Result<PreparedRun> {
            if self.fail_on == "prepare" {
                bail!("prepare failed")
            }
            Ok(PreparedRun::default())
        }

        fn launch(&self, _ctx: &AdapterContext, _prepared: &PreparedRun) -> Result<LaunchedRun> {
            if self.fail_on == "launch" {
                bail!("launch failed")
            }
            Ok(LaunchedRun {
                pid: Some(1),
                window_id: None,
                command: CommandSpec::default(),
                env: std::collections::BTreeMap::new(),
            })
        }

        fn collect(&self, _ctx: &AdapterContext, _run: &LaunchedRun) -> Result<CollectedData> {
            if self.fail_on == "collect" {
                bail!("collect failed")
            }
            Ok(CollectedData::default())
        }

        fn stop(&self, _ctx: &AdapterContext, _run: &LaunchedRun) -> Result<()> {
            if self.fail_on == "stop" {
                bail!("stop failed")
            }
            Ok(())
        }
    }

    struct AlwaysSucceedAdapter;

    impl TargetAdapter for AlwaysSucceedAdapter {
        fn id(&self) -> &'static str {
            "always_succeed"
        }

        fn discover_scenarios(&self) -> Vec<ScenarioRef> {
            vec![ScenarioRef { name: "test".to_string(), description: None }]
        }

        fn prepare(&self, _ctx: &AdapterContext, _spec: &ScenarioSpec) -> Result<PreparedRun> {
            Ok(PreparedRun::default())
        }

        fn launch(&self, _ctx: &AdapterContext, _prepared: &PreparedRun) -> Result<LaunchedRun> {
            Ok(LaunchedRun {
                pid: Some(1),
                window_id: None,
                command: CommandSpec::default(),
                env: std::collections::BTreeMap::new(),
            })
        }

        fn collect(&self, _ctx: &AdapterContext, _run: &LaunchedRun) -> Result<CollectedData> {
            Ok(CollectedData::default())
        }

        fn stop(&self, _ctx: &AdapterContext, _run: &LaunchedRun) -> Result<()> {
            Ok(())
        }
    }

    fn test_ctx() -> AdapterContext {
        AdapterContext::new(
            PathBuf::from("/test"),
            PathBuf::from("/tmp"),
            serde_json::json!({}),
        )
    }

    fn test_spec() -> ScenarioSpec {
        ScenarioSpec {
            name: "test".to_string(),
            config: serde_json::json!({}),
        }
    }

    #[test]
    fn lifecycle_result_success() {
        let result = LifecycleResult::success(CollectedData::default());
        assert!(result.is_success());
        let cloned = LifecycleResult::success(CollectedData::default());
        assert!(cloned.ok().is_some());
        let cloned2 = LifecycleResult::success(CollectedData::default());
        assert!(cloned2.err().is_none());
        assert!(result.cleanup_err().is_none());
    }

    #[test]
    fn lifecycle_result_failure() {
        let err = anyhow::anyhow!("test error");
        let result: LifecycleResult<CollectedData> = LifecycleResult::failure(err);
        assert!(!result.is_success());
        let cloned = LifecycleResult::<CollectedData>::failure(anyhow::anyhow!("test error"));
        assert!(cloned.ok().is_none());
        let cloned2 = LifecycleResult::<CollectedData>::failure(anyhow::anyhow!("test error"));
        assert!(cloned2.err().is_some());
    }

    #[test]
    fn run_lifecycle_succeeds() {
        let adapter = AlwaysSucceedAdapter;
        let ctx = test_ctx();
        let spec = test_spec();

        let result = run_lifecycle(&adapter, &ctx, &spec);
        assert!(result.is_success());
        assert!(result.cleanup_called);
        assert!(result.cleanup_error.is_none());
    }

    #[test]
    fn run_lifecycle_fails_on_prepare() {
        let adapter = FailingAdapter::new("prepare");
        let ctx = test_ctx();
        let spec = test_spec();

        let result = run_lifecycle(&adapter, &ctx, &spec);
        assert!(!result.is_success());
        assert!(!result.cleanup_called); // prepare didn't succeed, so no launch
    }

    #[test]
    fn run_lifecycle_fails_on_launch() {
        let adapter = FailingAdapter::new("launch");
        let ctx = test_ctx();
        let spec = test_spec();

        let result = run_lifecycle(&adapter, &ctx, &spec);
        assert!(!result.is_success());
        // Nothing was launched, so stop was not called
        assert!(!result.cleanup_called);
        assert!(result.cleanup_error.is_none());
    }

    #[test]
    fn run_lifecycle_fails_on_collect() {
        let adapter = FailingAdapter::new("collect");
        let ctx = test_ctx();
        let spec = test_spec();

        let result = run_lifecycle(&adapter, &ctx, &spec);
        assert!(!result.is_success());
        assert!(result.cleanup_called);
        assert!(result.cleanup_error.is_none());
    }

    #[test]
    fn run_lifecycle_collect_ignores_launched_run() {
        let adapter = AlwaysSucceedAdapter;
        let ctx = test_ctx();
        let spec = test_spec();

        let result = run_lifecycle_collect(&adapter, &ctx, &spec);
        assert!(result.is_success());
        // The result should be CollectedData, not (LaunchedRun, CollectedData)
        assert!(result.ok().is_some());
    }

    #[test]
    fn lifecycle_error_display() {
        let err = LifecycleError {
            original: "original error".to_string(),
            cleanup: Some("cleanup error".to_string()),
        };
        let display = format!("{}", err);
        assert!(display.contains("original error"));
        assert!(display.contains("cleanup error"));
    }

    #[test]
    fn lifecycle_result_cleanup_error() {
        let adapter = FailingAdapter::new("stop");
        let ctx = test_ctx();
        let spec = test_spec();

        let result = run_lifecycle(&adapter, &ctx, &spec);
        // The lifecycle succeeded (collect worked), but cleanup failed
        assert!(result.is_success());
        assert!(result.cleanup_called);
        assert!(result.cleanup_error.is_some());
    }

    // Adapter that fails on both collect and stop phases
    struct FailCollectAndStopAdapter;

    impl TargetAdapter for FailCollectAndStopAdapter {
        fn id(&self) -> &'static str {
            "fail_collect_and_stop"
        }

        fn discover_scenarios(&self) -> Vec<ScenarioRef> {
            vec![ScenarioRef { name: "test".to_string(), description: None }]
        }

        fn prepare(
            &self,
            _ctx: &AdapterContext,
            _spec: &ScenarioSpec,
        ) -> Result<PreparedRun> {
            Ok(PreparedRun::default())
        }

        fn launch(&self, _ctx: &AdapterContext, _prepared: &PreparedRun) -> Result<LaunchedRun> {
            Ok(LaunchedRun {
                pid: Some(1),
                window_id: None,
                command: CommandSpec::default(),
                env: std::collections::BTreeMap::new(),
            })
        }

        fn collect(&self, _ctx: &AdapterContext, _run: &LaunchedRun) -> Result<CollectedData> {
            bail!("collect failed")
        }

        fn stop(&self, _ctx: &AdapterContext, _run: &LaunchedRun) -> Result<()> {
            bail!("stop also failed")
        }
    }

    #[test]
    fn lifecycle_result_preserves_error_when_cleanup_fails() {
        // When both the main operation fails AND cleanup fails,
        // the original error is preserved and cleanup error is captured separately
        let adapter = FailCollectAndStopAdapter;
        let ctx = test_ctx();
        let spec = test_spec();

        let result = run_lifecycle(&adapter, &ctx, &spec);
        // Main operation failed
        assert!(!result.is_success());
        // Cleanup was called
        assert!(result.cleanup_called);
        // Both errors should be preserved
        assert!(result.cleanup_error.is_some());
        let cleanup_err = result.cleanup_error.unwrap();
        assert!(cleanup_err.contains("stop also failed"));
    }
}
