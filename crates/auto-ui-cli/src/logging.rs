use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing with optional JSON log mode.
/// When AUTO_UI_JSON_LOG env var is set, logs are emitted as JSON.
pub fn init() {
    let json_mode = std::env::var("AUTO_UI_JSON_LOG").is_ok();

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,auto_ui=debug"));

    if json_mode {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(true).with_thread_ids(true))
            .init();
    }
}

/// Returns true if JSON log mode is enabled via AUTO_UI_JSON_LOG env var.
pub fn is_json_log_mode() -> bool {
    std::env::var("AUTO_UI_JSON_LOG").is_ok()
}

#[cfg(test)]
mod tests {
    use tracing::info;

    #[test]
    fn logging_init_does_not_panic() {
        // Just verify init runs without panicking.
        // Note: init() may be called multiple times in tests (once per test),
        // but tracing-subscriber handles this gracefully.
        let _ = super::init();
        info!("logging initialized successfully");
    }

    #[test]
    fn json_log_mode_detection() {
        // When AUTO_UI_JSON_LOG is not set, should return false
        std::env::remove_var("AUTO_UI_JSON_LOG");
        assert!(!super::is_json_log_mode());

        // When AUTO_UI_JSON_LOG is set (to anything), should return true
        std::env::set_var("AUTO_UI_JSON_LOG", "1");
        assert!(super::is_json_log_mode());

        // Also true for other values
        std::env::set_var("AUTO_UI_JSON_LOG", "true");
        assert!(super::is_json_log_mode());

        std::env::remove_var("AUTO_UI_JSON_LOG");
    }
}
