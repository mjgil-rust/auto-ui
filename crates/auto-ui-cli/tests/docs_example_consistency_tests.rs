//! Docs/example consistency tests for CLI.
//!
//! These tests verify that example TOML files are valid and that
//! documented commands match the actual CLI surface.

use std::path::PathBuf;

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
}

fn parse_and_validate(path: &PathBuf) -> Result<(), String> {
    let scenario_file = auto_ui_core::parse_scenario_file(path).map_err(|e| e.to_string())?;
    let target = auto_ui_core::normalize_name(&scenario_file.target);
    let scenario = auto_ui_core::normalize_name(&scenario_file.scenario);
    auto_ui_cli::run_cmd::validate_selected_scenario(&target, &scenario, &scenario_file.value)
        .map_err(|e| e.to_string())
}

#[test]
fn rust_chatbot_debug_example_validates() {
    let path = example_dir().join("rust-chatbot-debug.toml");
    parse_and_validate(&path).expect("rust-chatbot-debug.toml should validate");
}

#[test]
fn rust_chatbot_header_debug_example_validates() {
    let path = example_dir().join("rust-chatbot-header-debug.toml");
    parse_and_validate(&path).expect("rust-chatbot-header-debug.toml should validate");
}

#[test]
fn rust_chatbot_prompt_debug_example_validates() {
    let path = example_dir().join("rust-chatbot-prompt-debug.toml");
    parse_and_validate(&path).expect("rust-chatbot-prompt-debug.toml should validate");
}

#[test]
fn gpui_scroll_matrix_example_validates() {
    let path = example_dir().join("gpui-scroll-matrix.toml");
    parse_and_validate(&path).expect("gpui-scroll-matrix.toml should validate");
}

#[test]
fn gpui_scrollbar_trace_example_validates() {
    let path = example_dir().join("gpui-scrollbar-trace.toml");
    parse_and_validate(&path).expect("gpui-scrollbar-trace.toml should validate");
}

#[test]
fn gpui_conversation_paint_example_validates() {
    let path = example_dir().join("gpui-conversation-paint.toml");
    parse_and_validate(&path).expect("gpui-conversation-paint.toml should validate");
}

#[test]
fn all_example_tomls_are_listed_in_readme() {
    let readme = std::fs::read_to_string(example_dir().join("README.md"))
        .expect("examples/README.md should be readable");

    let expected_tomls = [
        "rust-chatbot-debug.toml",
        "rust-chatbot-header-debug.toml",
        "rust-chatbot-prompt-debug.toml",
        "gpui-scroll-matrix.toml",
        "gpui-scrollbar-trace.toml",
        "gpui-conversation-paint.toml",
    ];

    for toml in expected_tomls {
        assert!(
            readme.contains(toml),
            "examples/README.md should list {}",
            toml
        );
    }
}

#[test]
fn all_example_tomls_exist_on_disk() {
    let tomls = [
        "rust-chatbot-debug.toml",
        "rust-chatbot-header-debug.toml",
        "rust-chatbot-prompt-debug.toml",
        "gpui-scroll-matrix.toml",
        "gpui-scrollbar-trace.toml",
        "gpui-conversation-paint.toml",
    ];

    for toml in tomls {
        let path = example_dir().join(toml);
        assert!(path.exists(), "{} should exist at {}", toml, path.display());
    }
}

#[test]
fn help_documents_all_convenience_commands() {
    let help_doc = std::fs::read_to_string(example_dir().join("help.md"))
        .expect("examples/help.md should be readable");

    // Verify all convenience commands appear in help docs
    assert!(
        help_doc.contains("debug"),
        "help.md should document debug command"
    );
    assert!(
        help_doc.contains("header-debug"),
        "help.md should document header-debug command"
    );
}

#[test]
fn gpui_docs_list_all_scenarios() {
    let gpui_doc = std::fs::read_to_string(example_dir().join("gpui-component-testing.md"))
        .expect("gpui-component-testing.md should be readable");

    assert!(
        gpui_doc.contains("Scroll matrix") || gpui_doc.contains("scroll-matrix"),
        "gpui-component-testing.md should document scroll_matrix"
    );
    assert!(
        gpui_doc.contains("Scrollbar trace") || gpui_doc.contains("scrollbar-trace"),
        "gpui-component-testing.md should document scrollbar_trace"
    );
    assert!(
        gpui_doc.contains("Conversation paint") || gpui_doc.contains("conversation-paint"),
        "gpui-component-testing.md should document conversation_paint"
    );
}

#[test]
fn rust_chatbot_docs_list_all_scenarios() {
    let debug_doc = std::fs::read_to_string(example_dir().join("rust-chatbot-debug.md"))
        .expect("rust-chatbot-debug.md should be readable");
    let header_doc = std::fs::read_to_string(example_dir().join("rust-chatbot-header-debug.md"))
        .expect("rust-chatbot-header-debug.md should be readable");
    let prompt_doc = std::fs::read_to_string(example_dir().join("rust-chatbot-prompt-debug.md"))
        .expect("rust-chatbot-prompt-debug.md should be readable");

    // Each doc should reference its scenario
    assert!(
        debug_doc.contains("debug") || debug_doc.contains("width"),
        "rust-chatbot-debug.md should document debug scenario"
    );
    assert!(
        header_doc.contains("header") || header_doc.contains("header_debug"),
        "rust-chatbot-header-debug.md should document header-debug scenario"
    );
    assert!(
        prompt_doc.contains("prompt") || prompt_doc.contains("prompt_debug"),
        "rust-chatbot-prompt-debug.md should document prompt-debug scenario"
    );
}
