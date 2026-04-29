//! Shared retry utilities for operations that may require polling or waiting.
//!
//! This module provides configurable retry logic for operations like
//! window discovery and trace waiting that may not succeed immediately.

use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Configuration for retry behavior.
#[derive(Clone, Debug)]
pub struct RetryConfig {
    /// Maximum number of attempts before giving up.
    pub max_attempts: u32,
    /// Initial delay between attempts.
    pub initial_delay: Duration,
    /// Maximum delay between attempts.
    pub max_delay: Duration,
    /// Multiplier for exponential backoff.
    pub backoff_multiplier: f64,
    /// Total time budget for all retries.
    pub timeout: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
            backoff_multiplier: 1.5,
            timeout: Duration::from_secs(30),
        }
    }
}

impl RetryConfig {
    /// Creates a config with a custom max attempts.
    pub fn with_max_attempts(mut self, n: u32) -> Self {
        self.max_attempts = n;
        self
    }

    /// Creates a config with a custom initial delay.
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Creates a config with a custom timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Creates a config with a custom max delay.
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }
}

/// A retryable operation that returns Ok(()) when successful.
pub type RetryOp = dyn Fn() -> std::result::Result<(), RetryError>;

/// Error type for retry operations.
#[derive(Debug, Clone)]
pub struct RetryError {
    pub message: String,
    pub transient: bool,
}

impl RetryError {
    /// Creates a transient error (may succeed on retry).
    pub fn transient(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            transient: true,
        }
    }

    /// Creates a permanent error (will never succeed on retry).
    pub fn permanent(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            transient: false,
        }
    }
}

impl std::fmt::Display for RetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RetryError {}

/// Runs a retry loop with exponential backoff.
pub fn retry_with_backoff(
    config: &RetryConfig,
    operation: &RetryOp,
    op_name: &str,
) -> Result<(), RetryError> {
    let start = Instant::now();
    let mut delay = config.initial_delay;
    let mut attempt = 0u32;

    loop {
        attempt += 1;

        match operation() {
            Ok(()) => {
                debug!(
                    attempt = attempt,
                    elapsed_ms = start.elapsed().as_millis() as u64,
                    "{} succeeded",
                    op_name
                );
                return Ok(());
            }
            Err(ref e) if !e.transient => {
                // Permanent error - don't retry
                warn!(attempt = attempt, error = %e, "{} failed permanently", op_name);
                return Err(e.clone());
            }
            Err(ref e) => {
                // Transient error - may retry
                if attempt >= config.max_attempts {
                    warn!(attempt = attempt, error = %e, "{} exhausted retries", op_name);
                    return Err(e.clone());
                }

                if start.elapsed() + delay > config.timeout {
                    warn!(
                        attempt = attempt,
                        elapsed_ms = start.elapsed().as_millis() as u64,
                        timeout_ms = config.timeout.as_millis() as u64,
                        error = %e,
                        "{} timed out during retry",
                        op_name
                    );
                    return Err(e.clone());
                }

                debug!(
                    attempt = attempt,
                    delay_ms = delay.as_millis() as u64,
                    error = %e,
                    "{} retrying after backoff",
                    op_name
                );
                std::thread::sleep(delay);

                // Exponential backoff with cap
                let new_delay =
                    Duration::from_secs_f64(delay.as_secs_f64() * config.backoff_multiplier);
                delay = new_delay.min(config.max_delay);
            }
        }
    }
}

/// Helper to create a retry config for window discovery.
pub fn window_discovery_config(timeout_secs: u64) -> RetryConfig {
    RetryConfig::default()
        .with_timeout(Duration::from_secs(timeout_secs))
        .with_max_attempts(((timeout_secs * 10) as u32).max(50))
        .with_initial_delay(Duration::from_millis(200))
}

/// Helper to create a retry config for trace waiting.
pub fn trace_wait_config(timeout_secs: u64) -> RetryConfig {
    RetryConfig::default()
        .with_timeout(Duration::from_secs(timeout_secs))
        .with_max_attempts(((timeout_secs * 5) as u32).max(20))
        .with_initial_delay(Duration::from_millis(500))
        .with_max_delay(Duration::from_secs(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_config_default_values() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 10);
        assert_eq!(config.initial_delay, Duration::from_millis(100));
        assert_eq!(config.max_delay, Duration::from_secs(2));
        assert!(config.backoff_multiplier > 1.0);
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn retry_config_builder() {
        let config = RetryConfig::default()
            .with_max_attempts(5)
            .with_timeout(Duration::from_secs(10))
            .with_initial_delay(Duration::from_millis(50));

        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.timeout, Duration::from_secs(10));
        assert_eq!(config.initial_delay, Duration::from_millis(50));
    }

    #[test]
    fn retry_error_transient() {
        let err = RetryError::transient("temporary failure");
        assert!(err.transient);
        assert_eq!(err.message, "temporary failure");
    }

    #[test]
    fn retry_error_permanent() {
        let err = RetryError::permanent("fatal failure");
        assert!(!err.transient);
        assert_eq!(err.message, "fatal failure");
    }

    #[test]
    fn retry_succeeds_on_first_attempt() {
        static CALL_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        CALL_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);

        let operation = || {
            CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        };

        let config = RetryConfig::default();
        let result = retry_with_backoff(&config, &operation, "test_op");

        assert!(result.is_ok());
        assert_eq!(CALL_COUNT.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn retry_succeeds_after_few_failures() {
        static CALL_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        CALL_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);

        let operation = || {
            let count = CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count < 3 {
                Err(RetryError::transient("not ready yet"))
            } else {
                Ok(())
            }
        };

        let config = RetryConfig::default().with_max_attempts(10);
        let result = retry_with_backoff(&config, &operation, "test_op");

        assert!(result.is_ok());
        assert_eq!(CALL_COUNT.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    #[test]
    fn retry_gives_up_after_max_attempts() {
        static CALL_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        CALL_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);

        let operation = || {
            CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(RetryError::transient("always fails"))
        };

        let config = RetryConfig::default().with_max_attempts(5);
        let result = retry_with_backoff(&config, &operation, "test_op");

        assert!(result.is_err());
        assert_eq!(CALL_COUNT.load(std::sync::atomic::Ordering::SeqCst), 5);
    }

    #[test]
    fn retry_stops_on_permanent_error() {
        static CALL_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        CALL_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);

        let operation = || {
            CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(RetryError::permanent("cannot recover"))
        };

        let config = RetryConfig::default().with_max_attempts(10);
        let result = retry_with_backoff(&config, &operation, "test_op");

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.transient);
        // Should not retry on permanent error
        assert_eq!(CALL_COUNT.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn window_discovery_config_values() {
        let config = window_discovery_config(15);
        assert!(config.timeout >= Duration::from_secs(15));
        assert!(config.max_attempts >= 50);
        assert_eq!(config.initial_delay, Duration::from_millis(200));
    }

    #[test]
    fn trace_wait_config_values() {
        let config = trace_wait_config(30);
        assert!(config.timeout >= Duration::from_secs(30));
        assert!(config.max_attempts >= 20);
        assert_eq!(config.initial_delay, Duration::from_millis(500));
        assert_eq!(config.max_delay, Duration::from_secs(1));
    }

    #[test]
    fn retry_timeout_respected() {
        static CALL_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        CALL_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);

        let operation = || {
            CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // Small sleep to allow timeout to trigger
            std::thread::sleep(Duration::from_millis(50));
            Err(RetryError::transient("slow operation"))
        };

        // Very short timeout
        let config = RetryConfig::default()
            .with_timeout(Duration::from_millis(100))
            .with_max_attempts(100)
            .with_initial_delay(Duration::from_millis(20));

        let result = retry_with_backoff(&config, &operation, "test_op");

        assert!(result.is_err());
        // Should have tried several times within 100ms
        let count = CALL_COUNT.load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            count >= 2,
            "expected at least 2 attempts in 100ms, got {}",
            count
        );
    }
}
