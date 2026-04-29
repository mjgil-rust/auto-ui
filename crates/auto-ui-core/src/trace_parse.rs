//! Trace parsing utilities for rust-chatbot ui_auto_debug traces.
//!
//! This module provides parsers for different trace line formats
//! emitted by rust-chatbot during debug sessions.

use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::TraceFields;

/// Known trace line types.
#[derive(Clone, Debug, PartialEq)]
pub enum TraceLineKind {
    /// A code block trace line: ui_auto_debug_code_block
    CodeBlock,
    /// A session debug line: ui_auto_debug with session_id
    SessionDebug,
    /// An ai_response_end line
    AiResponseEnd,
    /// A markdown row line
    MarkdownRow,
    /// Unknown line type
    Unknown,
}

/// Identifies the kind of trace line.
pub fn trace_line_kind(line: &str) -> TraceLineKind {
    if line.contains("ui_auto_debug_code_block") {
        TraceLineKind::CodeBlock
    } else if line.contains("ui_auto_debug") && line.contains("session_id=") {
        TraceLineKind::SessionDebug
    } else if line.contains("ai_response_end") {
        TraceLineKind::AiResponseEnd
    } else if line.contains("markdown") && line.contains("row") {
        TraceLineKind::MarkdownRow
    } else {
        TraceLineKind::Unknown
    }
}

/// Parses a trace line into key-value fields.
///
/// Supports both quoted and unquoted values:
/// - `session_id="abc123"` -> key: "session_id", value: "abc123"
/// - `session_id=abc123` -> key: "session_id", value: "abc123"
pub fn parse_trace_fields(line: &str) -> TraceFields {
    static FIELD_RE: OnceLock<Regex> = OnceLock::new();
    let re = FIELD_RE.get_or_init(|| Regex::new(r#"(\w+)=((?:"[^"]*")|(?:\S+))"#).unwrap());
    let mut fields = BTreeMap::new();
    for capture in re.captures_iter(line) {
        let key = capture.get(1).unwrap().as_str();
        let mut value = capture.get(2).unwrap().as_str().to_string();
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
        }
        fields.insert(key.to_string(), value);
    }
    fields
}

/// Extracts the session_id from a trace line.
pub fn extract_session_id(line: &str) -> Option<String> {
    let fields = parse_trace_fields(line);
    fields.get("session_id").cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_line_kind_code_block() {
        let line = "2024-01-15T10:30:00.000Z ui_auto_debug_code_block session_id=\"abc\" index=1";
        assert_eq!(trace_line_kind(line), TraceLineKind::CodeBlock);
    }

    #[test]
    fn trace_line_kind_session_debug() {
        let line = "2024-01-15T10:30:00.000Z ui_auto_debug session_id=\"abc\" action=start";
        assert_eq!(trace_line_kind(line), TraceLineKind::SessionDebug);
    }

    #[test]
    fn trace_line_kind_ai_response_end() {
        let line = "2024-01-15T10:30:00.000Z ai_response_end session_id=\"abc\" duration_ms=150";
        assert_eq!(trace_line_kind(line), TraceLineKind::AiResponseEnd);
    }

    #[test]
    fn trace_line_kind_markdown_row() {
        let line = "2024-01-15T10:30:00.000Z markdown row session_id=\"abc\" index=1";
        assert_eq!(trace_line_kind(line), TraceLineKind::MarkdownRow);
    }

    #[test]
    fn trace_line_kind_unknown() {
        let line = "2024-01-15T10:30:00.000Z some other log message";
        assert_eq!(trace_line_kind(line), TraceLineKind::Unknown);
    }

    #[test]
    fn parse_trace_fields_quoted_value() {
        let line = r#"session_id="abc123" action="start" index=1"#;
        let fields = parse_trace_fields(line);

        assert_eq!(fields.get("session_id"), Some(&"abc123".to_string()));
        assert_eq!(fields.get("action"), Some(&"start".to_string()));
        assert_eq!(fields.get("index"), Some(&"1".to_string()));
    }

    #[test]
    fn parse_trace_fields_unquoted_value() {
        let line = r#"session_id=abc123 action=start index=1"#;
        let fields = parse_trace_fields(line);

        assert_eq!(fields.get("session_id"), Some(&"abc123".to_string()));
        assert_eq!(fields.get("action"), Some(&"start".to_string()));
        assert_eq!(fields.get("index"), Some(&"1".to_string()));
    }

    #[test]
    fn parse_trace_fields_mixed_quoting() {
        let line = r#"session_id="abc-123" token=sk-ant-key value=plain_value"#;
        let fields = parse_trace_fields(line);

        assert_eq!(fields.get("session_id"), Some(&"abc-123".to_string()));
        assert_eq!(fields.get("token"), Some(&"sk-ant-key".to_string()));
        assert_eq!(fields.get("value"), Some(&"plain_value".to_string()));
    }

    #[test]
    fn parse_trace_fields_empty_line() {
        let line = "";
        let fields = parse_trace_fields(line);
        assert!(fields.is_empty());
    }

    #[test]
    fn parse_trace_fields_no_values() {
        let line = "just a log message with no fields";
        let fields = parse_trace_fields(line);
        assert!(fields.is_empty());
    }

    #[test]
    fn parse_trace_fields_with_special_chars() {
        let line = r#"session_id="abc 123" path="/tmp/test with spaces""#;
        let fields = parse_trace_fields(line);

        assert_eq!(fields.get("session_id"), Some(&"abc 123".to_string()));
        assert_eq!(
            fields.get("path"),
            Some(&"/tmp/test with spaces".to_string())
        );
    }

    #[test]
    fn extract_session_id_present() {
        let line = r#"2024-01-15T10:30:00.000Z ui_auto_debug session_id="abc123" action=start"#;
        let session_id = extract_session_id(line);
        assert_eq!(session_id, Some("abc123".to_string()));
    }

    #[test]
    fn extract_session_id_missing() {
        let line = r#"2024-01-15T10:30:00.000Z some other log message"#;
        let session_id = extract_session_id(line);
        assert!(session_id.is_none());
    }

    #[test]
    fn parse_trace_fields_code_block_line() {
        let line = r#"2024-01-15T10:30:00.000Z ui_auto_debug_code_block session_id="s1" index=0 block_type="markdown""#;
        let fields = parse_trace_fields(line);

        assert_eq!(fields.get("session_id"), Some(&"s1".to_string()));
        assert_eq!(fields.get("index"), Some(&"0".to_string()));
        assert_eq!(fields.get("block_type"), Some(&"markdown".to_string()));
    }

    #[test]
    fn parse_trace_fields_with_numbers() {
        let line = r#"timestamp=1705312200000 session_id="abc" width=1280 height=800 count=42"#;
        let fields = parse_trace_fields(line);

        assert_eq!(fields.get("session_id"), Some(&"abc".to_string()));
        assert_eq!(fields.get("timestamp"), Some(&"1705312200000".to_string()));
        assert_eq!(fields.get("width"), Some(&"1280".to_string()));
        assert_eq!(fields.get("count"), Some(&"42".to_string()));
    }

    #[test]
    fn parse_trace_fields_empty_quoted_value() {
        let line = r#"session_id="" action=start"#;
        let fields = parse_trace_fields(line);
        assert_eq!(fields.get("session_id"), Some(&"".to_string()));
        assert_eq!(fields.get("action"), Some(&"start".to_string()));
    }

    #[test]
    fn parse_trace_fields_malformed_incomplete_quote() {
        let line = r#"session_id="abc action=start"#;
        let fields = parse_trace_fields(line);
        // Should capture what it can - the regex stops at whitespace
        assert_eq!(fields.get("session_id"), Some(&"\"abc".to_string()));
    }

    #[test]
    fn parse_trace_fields_only_whitespace() {
        let line = "   ";
        let fields = parse_trace_fields(line);
        assert!(fields.is_empty());
    }

    #[test]
    fn parse_trace_fields_duplicate_keys_takes_last() {
        let line = r#"session_id="first" session_id="last""#;
        let fields = parse_trace_fields(line);
        assert_eq!(fields.get("session_id"), Some(&"last".to_string()));
    }

    #[test]
    fn trace_line_kind_with_embedded_quotes_in_value() {
        // Quotes embedded in values are captured literally since regex doesn't handle escaped quotes
        let line = r#"session_id="abc" path="file with "" quotes""#;
        let fields = parse_trace_fields(line);
        // The regex captures until whitespace or end, so embedded quotes become part of value
        assert_eq!(fields.get("session_id"), Some(&"abc".to_string()));
        // Path value includes the embedded quotes
        assert!(fields.get("path").is_some());
    }
}
