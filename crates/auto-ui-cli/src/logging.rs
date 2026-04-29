use std::env;

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init() {
    let json_mode = env::var("AUTO_UI_JSON_LOG").is_ok();

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
}
