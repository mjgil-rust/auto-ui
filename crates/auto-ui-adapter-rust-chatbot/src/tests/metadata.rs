use super::*;

fn create_mock_sessions_map(
    provider: Provider,
    temp_dir: &Path,
    sessions_json: &str,
) -> Map<String, Value> {
    let data_dir = temp_dir.join(match provider {
        Provider::Claude => ".claude-desktop",
        Provider::Codex => ".codex-desktop",
        Provider::Gemini => ".gemini-desktop",
        Provider::GeminiForge => ".gemini-forge-desktop",
        Provider::GeminiCliForge => ".gemini-cli-forge-desktop",
        Provider::MiniMaxForge => ".minimax-forge-desktop",
        Provider::OmniForge => ".omniforge-desktop",
    });
    fs::create_dir_all(&data_dir).unwrap();
    let metadata_path = data_dir.join("sessions.json");
    fs::write(&metadata_path, sessions_json).unwrap();
    parse_sessions_metadata_from_path(&metadata_path).unwrap()
}

#[test]
fn read_sessions_metadata_parses_valid_json() {
    let temp = unique_temp_dir("metadata-valid");
    let sessions_json = serde_json::json!({
        "sessions": {
            "abc123": {
                "id": "abc123",
                "name": "Test Session",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 42,
                "hidden": false
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Codex,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    assert_eq!(sessions_map.len(), 1);
    assert!(sessions_map.contains_key("abc123"));
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn read_sessions_metadata_missing_sessions_object() {
    let temp = unique_temp_dir("metadata-missing-sessions");
    let data_dir = temp.join(".claude-desktop");
    fs::create_dir_all(&data_dir).unwrap();
    let metadata_path = data_dir.join("sessions.json");
    fs::write(&metadata_path, r#"{"something_else": "not_sessions"}"#).unwrap();
    let result = parse_sessions_metadata_from_path(&metadata_path);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("does not contain a sessions object"));
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn read_sessions_metadata_malformed_json() {
    let temp = unique_temp_dir("metadata-malformed");
    let data_dir = temp.join(".gemini-desktop");
    fs::create_dir_all(&data_dir).unwrap();
    let metadata_path = data_dir.join("sessions.json");
    fs::write(&metadata_path, "this is not json {{{").unwrap();
    let result = parse_sessions_metadata_from_path(&metadata_path);
    assert!(result.is_err());
    std::fs::remove_dir_all(temp).ok();
}
#[test]
fn read_sessions_metadata_all_provider_types() {
    // Test that all provider types can be parsed
    for provider in [
        Provider::Claude,
        Provider::Codex,
        Provider::Gemini,
        Provider::GeminiForge,
        Provider::GeminiCliForge,
        Provider::MiniMaxForge,
        Provider::OmniForge,
    ] {
        let temp = unique_temp_dir(&format!("metadata-{provider:?}"));
        let sessions_json = serde_json::json!({
            "sessions": {}
        });
        let sessions_map = create_mock_sessions_map(
            provider,
            &temp,
            &serde_json::to_string(&sessions_json).unwrap(),
        );
        assert!(sessions_map.is_empty(), "failed for {:?}", provider);
        std::fs::remove_dir_all(temp).ok();
    }
}

#[test]
fn load_sessions_filters_hidden_sessions() {
    let temp = unique_temp_dir("sessions-hidden-filter");
    let sessions_json = serde_json::json!({
        "sessions": {
            "session1": {
                "id": "session1",
                "name": "Visible Session",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 10,
                "hidden": false
            },
            "session2": {
                "id": "session2",
                "name": "Hidden Session",
                "updated_at": "2024-01-15T10:31:00Z",
                "message_count": 5,
                "hidden": true
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Codex,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    let result = load_sessions_from_map(sessions_map, 10, false, None);
    assert!(result.is_ok());
    let sessions = result.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].name, "Visible Session");
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_sessions_includes_hidden_when_requested() {
    let temp = unique_temp_dir("sessions-include-hidden");
    let sessions_json = serde_json::json!({
        "sessions": {
            "session1": {
                "id": "session1",
                "name": "Visible Session",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 10,
                "hidden": false
            },
            "session2": {
                "id": "session2",
                "name": "Hidden Session",
                "updated_at": "2024-01-15T10:31:00Z",
                "message_count": 5,
                "hidden": true
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Codex,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    let result = load_sessions_from_map(sessions_map, 10, true, None);
    assert!(result.is_ok());
    let sessions = result.unwrap();
    assert_eq!(sessions.len(), 2);
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_sessions_filters_zero_message_count() {
    let temp = unique_temp_dir("sessions-zero-count-filter");
    let sessions_json = serde_json::json!({
        "sessions": {
            "session1": {
                "id": "session1",
                "name": "Active Session",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 10,
                "hidden": false
            },
            "session2": {
                "id": "session2",
                "name": "Empty Session",
                "updated_at": "2024-01-15T10:31:00Z",
                "message_count": 0,
                "hidden": false
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Codex,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    let result = load_sessions_from_map(sessions_map, 10, true, None);
    assert!(result.is_ok());
    let sessions = result.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].name, "Active Session");
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_sessions_respects_max_sessions() {
    let temp = unique_temp_dir("sessions-max-limit");
    let mut sessions = serde_json::Map::new();
    for i in 0..5 {
        let mut session = serde_json::Map::new();
        session.insert("id".to_string(), serde_json::json!(format!("session{}", i)));
        session.insert(
            "name".to_string(),
            serde_json::json!(format!("Session {}", i)),
        );
        session.insert(
            "updated_at".to_string(),
            serde_json::json!("2024-01-15T10:00:00Z"),
        );
        session.insert("message_count".to_string(), serde_json::json!(10 - i));
        session.insert("hidden".to_string(), serde_json::json!(false));
        sessions.insert(format!("session{}", i), serde_json::Value::Object(session));
    }
    let sessions_map = create_mock_sessions_map(
        Provider::Codex,
        &temp,
        &serde_json::to_string(&serde_json::json!({ "sessions": sessions })).unwrap(),
    );
    let result = load_sessions_from_map(sessions_map, 3, true, None);
    assert!(result.is_ok());
    let sessions = result.unwrap();
    assert_eq!(sessions.len(), 3);
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_sessions_selects_newest_by_updated_at() {
    let temp = unique_temp_dir("sessions-newest-first");
    let mut sessions = serde_json::Map::new();
    // Add sessions with different updated_at timestamps
    let timestamps = [
        "2024-01-15T10:00:00Z", // oldest
        "2024-01-17T12:00:00Z", // newest
        "2024-01-16T08:00:00Z", // middle
    ];
    for (i, ts) in timestamps.iter().enumerate() {
        let mut session = serde_json::Map::new();
        session.insert("id".to_string(), serde_json::json!(format!("session{}", i)));
        session.insert(
            "name".to_string(),
            serde_json::json!(format!("Session {}", i)),
        );
        session.insert("updated_at".to_string(), serde_json::json!(ts));
        session.insert("message_count".to_string(), serde_json::json!(10));
        session.insert("hidden".to_string(), serde_json::json!(false));
        sessions.insert(format!("session{}", i), serde_json::Value::Object(session));
    }
    let sessions_map = create_mock_sessions_map(
        Provider::Claude,
        &temp,
        &serde_json::to_string(&serde_json::json!({ "sessions": sessions })).unwrap(),
    );
    // With max_sessions=2, should return session1 (newest) and session2 (middle), ordered newest first
    let result = load_sessions_from_map(sessions_map, 2, true, None);
    assert!(result.is_ok());
    let sessions = result.unwrap();
    assert_eq!(sessions.len(), 2);
    // Should be ordered by updated_at descending (newest first)
    assert_eq!(sessions[0].session_id, "session1"); // 2024-01-17T12:00:00Z
    assert_eq!(sessions[1].session_id, "session2"); // 2024-01-16T08:00:00Z
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_session_by_id_found() {
    let temp = unique_temp_dir("session-by-id-found");
    let sessions_json = serde_json::json!({
        "sessions": {
            "abc123": {
                "id": "abc123",
                "name": "Test Session",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 42,
                "hidden": false
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Claude,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    let result = load_session_by_id_from_map(sessions_map, Provider::Claude, "abc123");
    assert!(result.is_ok());
    let session = result.unwrap();
    assert_eq!(session.session_id, "abc123");
    assert_eq!(session.name, "Test Session");
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_session_by_id_not_found() {
    let temp = unique_temp_dir("session-by-id-not-found");
    let sessions_json = serde_json::json!({
        "sessions": {
            "abc123": {
                "id": "abc123",
                "name": "Test Session",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 42,
                "hidden": false
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Claude,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    let result = load_session_by_id_from_map(sessions_map, Provider::Claude, "nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("was not found"));
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_session_by_id_missing_name_field() {
    let temp = unique_temp_dir("session-missing-name");
    let sessions_json = serde_json::json!({
        "sessions": {
            "abc123": {
                "id": "abc123",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 42,
                "hidden": false
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Gemini,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    let result = load_session_by_id_from_map(sessions_map, Provider::Gemini, "abc123");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("missing a name"));
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_session_by_name_found() {
    let temp = unique_temp_dir("session-by-name-found");
    let sessions_json = serde_json::json!({
        "sessions": {
            "abc123": {
                "id": "abc123",
                "name": "Test Session",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 42,
                "hidden": false
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Claude,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    let result =
        load_session_by_name_from_map(sessions_map, Provider::Claude, "Test Session", false);
    assert!(result.is_ok());
    let session = result.unwrap();
    assert_eq!(session.session_id, "abc123");
    assert_eq!(session.name, "Test Session");
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_session_by_name_not_found() {
    let temp = unique_temp_dir("session-by-name-not-found");
    let sessions_json = serde_json::json!({
        "sessions": {
            "abc123": {
                "id": "abc123",
                "name": "Test Session",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 42,
                "hidden": false
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Claude,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    let result =
        load_session_by_name_from_map(sessions_map, Provider::Claude, "Nonexistent", false);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("was not found"));
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_session_by_name_excludes_hidden() {
    let temp = unique_temp_dir("session-by-name-hidden");
    let sessions_json = serde_json::json!({
        "sessions": {
            "abc123": {
                "id": "abc123",
                "name": "Hidden Session",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 42,
                "hidden": true
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Codex,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    // With include_hidden=false, hidden session should not be found
    let result =
        load_session_by_name_from_map(sessions_map, Provider::Codex, "Hidden Session", false);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("was not found"));
    std::fs::remove_dir_all(temp).ok();
}

#[test]
fn load_session_by_name_includes_hidden_when_requested() {
    let temp = unique_temp_dir("session-by-name-include-hidden");
    let sessions_json = serde_json::json!({
        "sessions": {
            "abc123": {
                "id": "abc123",
                "name": "Hidden Session",
                "updated_at": "2024-01-15T10:30:00Z",
                "message_count": 42,
                "hidden": true
            }
        }
    });
    let sessions_map = create_mock_sessions_map(
        Provider::Gemini,
        &temp,
        &serde_json::to_string(&sessions_json).unwrap(),
    );
    // With include_hidden=true, hidden session should be found
    let result =
        load_session_by_name_from_map(sessions_map, Provider::Gemini, "Hidden Session", true);
    assert!(result.is_ok());
    let session = result.unwrap();
    assert_eq!(session.session_id, "abc123");
    assert_eq!(session.name, "Hidden Session");
    std::fs::remove_dir_all(temp).ok();
}
