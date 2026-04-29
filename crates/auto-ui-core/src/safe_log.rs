//! Safe logging utilities for structured output with redaction support.
//!
//! This module provides helpers for emitting command, environment, and directory
//! information safely, with automatic redaction of sensitive values.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

/// Environment variables that should never be logged.
const SENSITIVE_ENV_VARS: &[&str] = &[
    "RUST_CHATBOT_TOKEN",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GOOGLE_API_KEY",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "SESSION_TOKEN",
    "AUTH_TOKEN",
    "PASSWORD",
    "SECRET",
    "PRIVATE_KEY",
    "XAUTHORITY",
];

/// Filters environment variables, redacting sensitive values.
pub fn redact_env_vars() -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for (key, value) in env::vars() {
        if SENSITIVE_ENV_VARS.iter().any(|s| key.contains(s)) {
            result.insert(key, "[REDACTED]".to_string());
        } else {
            result.insert(key, value);
        }
    }
    result
}

/// Redacts a single environment variable value if the key is sensitive.
pub fn redact_env_var(key: &str, value: &str) -> String {
    if SENSITIVE_ENV_VARS.iter().any(|s| key.contains(s)) {
        "[REDACTED]".to_string()
    } else {
        value.to_string()
    }
}

/// Redacts sensitive args in a command (e.g., tokens, secrets).
pub fn redact_command_args(args: &[String]) -> Vec<String> {
    args.iter()
        .map(|arg| {
            // Check if arg looks like a secret (contains api key pattern, token, etc.)
            let lower = arg.to_lowercase();
            if lower.contains("token=")
                || lower.contains("key=")
                || lower.contains("secret")
                || lower.contains("password")
                || lower.contains("api_key")
                || (arg.len() > 20
                    && arg
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_'))
            {
                "[REDACTED]".to_string()
            } else {
                arg.clone()
            }
        })
        .collect()
}

/// Sanitizes a path for logging by replacing home directory with ~.
pub fn sanitize_path_for_log(path: &PathBuf) -> String {
    if let Ok(home) = env::var("HOME") {
        if let Some(stripped) = path.to_string_lossy().strip_prefix(&home) {
            return format!("~{}", stripped);
        }
    }
    path.to_string_lossy().to_string()
}

/// Sanitizes current directory for logging.
pub fn sanitize_cwd_for_log() -> String {
    sanitize_path_for_log(&PathBuf::from(env::current_dir().unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_env_vars_redacts_sensitive() {
        env::set_var("MY_SECRET_TOKEN", "abc123");
        env::set_var("NORMAL_VAR", "normal_value");

        let filtered = redact_env_vars();

        assert_eq!(
            filtered.get("MY_SECRET_TOKEN"),
            Some(&"[REDACTED]".to_string())
        );
        assert_eq!(
            filtered.get("NORMAL_VAR"),
            Some(&"normal_value".to_string())
        );

        env::remove_var("MY_SECRET_TOKEN");
        env::remove_var("NORMAL_VAR");
    }

    #[test]
    fn redact_env_vars_redacts_partial_match() {
        env::set_var("ANTHROPIC_API_KEY", "sk-ant-abc123");
        let filtered = redact_env_vars();
        assert_eq!(
            filtered.get("ANTHROPIC_API_KEY"),
            Some(&"[REDACTED]".to_string())
        );
        env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn redact_command_args_redacts_token_args() {
        let args = vec![
            "chatbot-ctl".to_string(),
            "session".to_string(),
            "token=abc123".to_string(),
        ];
        let redacted = redact_command_args(&args);
        assert_eq!(redacted[0], "chatbot-ctl");
        assert_eq!(redacted[2], "[REDACTED]");
    }

    #[test]
    fn redact_command_args_preserves_normal_args() {
        let args = vec![
            "chatbot-ctl".to_string(),
            "start".to_string(),
            "--width=1280".to_string(),
        ];
        let redacted = redact_command_args(&args);
        assert_eq!(redacted, args);
    }

    #[test]
    fn redact_command_args_redacts_long_alphanumeric() {
        // Long random-looking strings that might be tokens/keys
        let args = vec![
            "cmd".to_string(),
            "sk-ant-api03-verylongstringthatlookslikeakey1234567890".to_string(),
        ];
        let redacted = redact_command_args(&args);
        assert_eq!(redacted[1], "[REDACTED]");
    }

    #[test]
    fn sanitize_path_replaces_home() {
        env::set_var("HOME", "/home/user");
        let path = PathBuf::from("/home/user/projects/test");
        let sanitized = sanitize_path_for_log(&path);
        assert_eq!(sanitized, "~/projects/test");
        env::remove_var("HOME");
    }

    #[test]
    fn sanitize_path_preserves_absolute_outside_home() {
        env::set_var("HOME", "/home/user");
        let path = PathBuf::from("/var/log/test");
        let sanitized = sanitize_path_for_log(&path);
        assert_eq!(sanitized, "/var/log/test");
        env::remove_var("HOME");
    }

    #[test]
    fn sanitize_cwd_for_log_returns_string() {
        let cwd = sanitize_cwd_for_log();
        assert!(!cwd.is_empty());
    }
}
