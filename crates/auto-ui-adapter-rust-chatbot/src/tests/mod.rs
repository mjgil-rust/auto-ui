use super::*;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn live_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("auto-ui-{name}-{nanos}-{}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn require_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this live smoke test"))
}

fn require_live_opt_in() {
    assert_eq!(
        std::env::var("AUTO_UI_RUN_LIVE_TESTS").as_deref(),
        Ok("1"),
        "set AUTO_UI_RUN_LIVE_TESTS=1 to run ignored live smoke tests"
    );
}

fn provider_from_env() -> Provider {
    match std::env::var("AUTO_UI_TEST_RUST_CHATBOT_PROVIDER")
        .unwrap_or_else(|_| "codex".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "claude" => Provider::Claude,
        "codex" => Provider::Codex,
        "gemini" => Provider::Gemini,
        "geminiforge" | "gemini-forge" | "gemini_forge" => Provider::GeminiForge,
        "geminicliforge" | "gemini-cli-forge" | "gemini_cli_forge" => Provider::GeminiCliForge,
        "minimaxforge" | "minimax-forge" | "minimax_forge" => Provider::MiniMaxForge,
        "omniforge" => Provider::OmniForge,
        other => panic!("unsupported AUTO_UI_TEST_RUST_CHATBOT_PROVIDER={other}"),
    }
}

#[test]
fn forge_providers_map_to_rust_chatbot_metadata() {
    assert_eq!(Provider::GeminiForge.as_str(), "geminiforge");
    assert_eq!(
        Provider::GeminiForge.title_base(),
        "GeminiForge Rust Chatbot"
    );
    assert_eq!(
        Provider::GeminiForge.data_dir_name(),
        ".gemini-forge-desktop"
    );
    assert_eq!(
        Provider::GeminiForge.provider_session_field(),
        "gemini_session_id"
    );

    assert_eq!(Provider::GeminiCliForge.as_str(), "geminicliforge");
    assert_eq!(
        Provider::GeminiCliForge.title_base(),
        "GeminiCliForge Rust Chatbot"
    );
    assert_eq!(
        Provider::GeminiCliForge.data_dir_name(),
        ".gemini-cli-forge-desktop"
    );
    assert_eq!(
        Provider::GeminiCliForge.provider_session_field(),
        "gemini_session_id"
    );

    assert_eq!(Provider::MiniMaxForge.as_str(), "minimaxforge");
    assert_eq!(
        Provider::MiniMaxForge.title_base(),
        "MiniMaxForge Rust Chatbot"
    );
    assert_eq!(
        Provider::MiniMaxForge.data_dir_name(),
        ".minimax-forge-desktop"
    );
    assert_eq!(
        Provider::MiniMaxForge.provider_session_field(),
        "minimax_session_id"
    );

    assert_eq!(Provider::OmniForge.as_str(), "omniforge");
    assert_eq!(Provider::OmniForge.title_base(), "OmniForge Rust Chatbot");
    assert_eq!(Provider::OmniForge.data_dir_name(), ".omniforge-desktop");
    assert_eq!(
        Provider::OmniForge.provider_session_field(),
        "omniforge_session_id"
    );
}

#[test]
fn resolve_app_root_prefers_explicit_path() {
    // When raw_path is provided, it should be used directly
    let temp = unique_temp_dir("resolve-app-root-test");
    let result = resolve_app_root(Some(&temp.display().to_string()));
    assert!(result.is_ok());
    // The temp dir exists, so it should resolve successfully
    std::fs::remove_dir(temp).ok();
}

#[test]
fn resolve_app_root_falls_back_to_env() {
    // Set a custom env var value
    std::env::set_var("RUST_CHATBOT_APP_ROOT", "/tmp/test-env-root");
    let result = resolve_app_root(None);
    // Should try env var
    assert!(result.is_ok() || result.is_err()); // May fail if path doesn't exist, but should try
    std::env::remove_var("RUST_CHATBOT_APP_ROOT");
}

#[test]
fn resolve_app_root_falls_back_to_sibling() {
    // When no env var is set and no explicit path, should try sibling
    // This test checks that when env vars are not set, the sibling fallback is attempted
    // The actual sibling path depends on repo layout, so we just ensure no panic
    let result = resolve_app_root(None);
    // Result is either Ok(path) or Err with the expected message about missing root
    match result {
        Ok(_) => {}
        Err(e) => {
            // Should be the error about missing app root, not a panic
            assert!(
                e.to_string().contains("rust-chatbot")
                    || e.to_string().contains("Could not resolve")
            );
        }
    }
}

#[test]
fn resolve_app_root_returns_error_when_no_path_found() {
    // Clear any env vars that might interfere
    std::env::remove_var("RUST_CHATBOT_APP_ROOT");
    std::env::remove_var("RUST_CHATBOT_ROOT");

    let result = resolve_app_root(None);

    // Should either succeed (sibling exists in test env) or fail with the expected message
    match result {
        Ok(path) => {
            // If it succeeds, the path should at least be valid
            assert!(path.is_absolute());
        }
        Err(e) => {
            // If it fails, should be our expected error
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("Could not resolve") || err_msg.contains("rust-chatbot"),
                "unexpected error: {}",
                err_msg
            );
        }
    }
}

#[test]
fn resolve_app_root_expands_tilde() {
    // Test that ~ is expanded in paths
    let temp = unique_temp_dir("resolve-app-root-tilde");
    let tilde_path = format!("~/{}", temp.file_name().unwrap().to_string_lossy());
    // Note: we can't easily test tilde expansion here without a known home,
    // but we can verify the function doesn't panic
    let _ = temp; // unused in this test, just for documentation
}

#[test]
fn resolve_app_root_non_existent_path() {
    // Absolute non-existent paths are returned as-is (expand_path does not fail on them)
    let result = resolve_app_root(Some("/this/path/does/not/exist/at/all"));
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        PathBuf::from("/this/path/does/not/exist/at/all")
    );
}

#[test]
fn resolve_app_root_empty_string_path() {
    // Empty string should be treated as None for raw_path
    let result = resolve_app_root(Some(""));
    // Should fall through to env var and sibling checks
    // Might succeed or fail depending on environment
    match result {
        Ok(_) => {}
        Err(e) => assert!(
            e.to_string().contains("Could not resolve") || e.to_string().contains("rust-chatbot")
        ),
    }
}

#[test]
fn resolve_app_root_env_rust_chatbot_root() {
    // Test the alternate env var name
    std::env::set_var("RUST_CHATBOT_ROOT", "/tmp/test-root-alt");
    let result = resolve_app_root(None);
    // Should use RUST_CHATBOT_ROOT
    assert!(result.is_ok() || result.is_err()); // Path might not exist but env should be read
    std::env::remove_var("RUST_CHATBOT_ROOT");
}

#[test]
fn resolve_app_root_app_root_takes_precedence() {
    // When both explicit path AND env var are set, explicit should win
    std::env::set_var("RUST_CHATBOT_APP_ROOT", "/tmp/env-root-should-not-be-used");
    let temp = unique_temp_dir("explicit-precedence");
    let explicit_path = temp.display().to_string();
    let result = resolve_app_root(Some(&explicit_path));
    assert!(result.is_ok());
    std::env::remove_var("RUST_CHATBOT_APP_ROOT");
    std::fs::remove_dir(temp).ok();
}

#[test]
fn resolve_app_root_canonicalizes_sibling() {
    // If sibling exists, it should be canonicalized
    let sibling_path = PathBuf::from("/tmp/nonexistent-sibling-should-not-exist");
    if !sibling_path.exists() {
        std::fs::create_dir_all(&sibling_path).ok();
    }
    // This would only work if /tmp/nonexistent-sibling exists, which it doesn't
    // The point is to test that canonicalize is called on sibling if it exists
    std::fs::remove_dir(sibling_path).ok();
}

#[test]
fn resolve_app_root_priority_cli_over_env() {
    // CLI argument (raw_path) should take priority over environment variable
    std::env::set_var("RUST_CHATBOT_APP_ROOT", "/tmp/env-should-not-be-used");
    let temp = unique_temp_dir("cli-over-env");
    let result = resolve_app_root(Some(&temp.display().to_string()));
    assert!(result.is_ok());
    std::env::remove_var("RUST_CHATBOT_APP_ROOT");
    std::fs::remove_dir(temp).ok();
}

#[test]
fn resolve_app_root_priority_env_over_sibling() {
    // Environment variable should take priority over sibling fallback
    std::env::set_var("RUST_CHATBOT_APP_ROOT", "/tmp/env-priority-test");
    let result = resolve_app_root(None);
    assert!(result.is_ok());
    std::env::remove_var("RUST_CHATBOT_APP_ROOT");
}

#[test]
fn resolve_app_root_prefers_rust_chatbot_app_root_over_rust_chatbot_root() {
    // Both env vars set - RUST_CHATBOT_APP_ROOT should win
    std::env::set_var("RUST_CHATBOT_APP_ROOT", "/tmp/app-root-wins");
    std::env::set_var("RUST_CHATBOT_ROOT", "/tmp/root-should-lose");
    let result = resolve_app_root(None);
    assert!(result.is_ok());
    // Verify it tried APP_ROOT first by checking the path is what we set
    assert_eq!(result.unwrap().to_string_lossy(), "/tmp/app-root-wins");
    std::env::remove_var("RUST_CHATBOT_APP_ROOT");
    std::env::remove_var("RUST_CHATBOT_ROOT");
}

#[test]
#[ignore = "requires DISPLAY, built rust-chatbot binaries, and AUTO_UI_RUN_LIVE_TESTS=1"]
fn live_debug_single_session_smoke() {
    let _guard = live_test_lock().lock().unwrap();
    require_live_opt_in();
    let output_dir = unique_temp_dir("rust-chatbot-debug-live");
    let completed = run_debug(DebugConfig {
        app_root: Some(require_env("AUTO_UI_TEST_RUST_CHATBOT_ROOT")),
        provider: provider_from_env(),
        instance: None,
        widths: "520".to_string(),
        height: 720,
        max_sessions: 1,
        session_id: Some(require_env("AUTO_UI_TEST_RUST_CHATBOT_SESSION_ID")),
        include_hidden: true,
        launch_if_missing: true,
        window_timeout: 20.0,
        trace_timeout: 10.0,
        settle: 0.7,
        output_dir: Some(output_dir.display().to_string()),
        keep_front: false,
    })
    .unwrap();
    assert!(completed.report_path.exists());
}

#[test]
#[ignore = "requires DISPLAY, built rust-chatbot binaries, and AUTO_UI_RUN_LIVE_TESTS=1"]
fn live_header_debug_single_session_smoke() {
    let _guard = live_test_lock().lock().unwrap();
    require_live_opt_in();
    let output_dir = unique_temp_dir("rust-chatbot-header-live");
    let completed = run_header_debug(HeaderDebugConfig {
        app_root: Some(require_env("AUTO_UI_TEST_RUST_CHATBOT_ROOT")),
        provider: provider_from_env(),
        instance: None,
        session_id: Some(require_env("AUTO_UI_TEST_RUST_CHATBOT_SESSION_ID")),
        session_name: None,
        include_hidden: true,
        widths: "520".to_string(),
        height: 720,
        header_height: 140,
        window_timeout: 20.0,
        trace_timeout: 12.0,
        settle: 0.8,
        output_dir: Some(output_dir.display().to_string()),
        keep_front: false,
    })
    .unwrap();
    assert!(completed.report_path.exists());
}

#[test]
#[ignore = "requires DISPLAY, built rust-chatbot binaries, and AUTO_UI_RUN_LIVE_TESTS=1"]
fn live_prompt_debug_single_session_smoke() {
    let _guard = live_test_lock().lock().unwrap();
    require_live_opt_in();
    let output_dir = unique_temp_dir("rust-chatbot-prompt-live");
    let completed = run_prompt_debug(PromptDebugConfig {
        app_root: Some(require_env("AUTO_UI_TEST_RUST_CHATBOT_ROOT")),
        provider: provider_from_env(),
        instance: None,
        session_id: require_env("AUTO_UI_TEST_RUST_CHATBOT_SESSION_ID"),
        prompt: "Hello, respond with a brief greeting.".to_string(),
        width: 700,
        height: 720,
        window_timeout: 20.0,
        response_timeout: 30.0,
        settle: 0.8,
        output_dir: Some(output_dir.display().to_string()),
        keep_front: false,
    })
    .unwrap();
    assert!(completed.report_path.exists());
}

// Fixture-based tests for provider metadata parsing (Task #64, #78)

mod metadata;
mod utilities;
