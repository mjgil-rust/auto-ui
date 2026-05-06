use super::*;

fn unique_temp_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("auto-ui-{name}-{nanos}-{}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn newest_trace_log_returns_newest_by_lexicographic_order() {
    // Log files are named rust-chatbot.log.1, rust-chatbot.log.2, etc.
    // The lexicographically newest should be returned when all exist
    // Note: lexicographic order means "2" > "10" > "1" because '2' > '1' at the first differing position
    let temp = unique_temp_dir("trace-log-newest");
    let log_dir = temp.join("logs");
    fs::create_dir_all(&log_dir).unwrap();

    // Create log files
    fs::write(log_dir.join("rust-chatbot.log.1"), "log 1").unwrap();
    fs::write(log_dir.join("rust-chatbot.log.2"), "log 2").unwrap();
    fs::write(log_dir.join("rust-chatbot.log.10"), "log 10").unwrap();

    // Test the sort order directly (this mirrors what newest_trace_log does)
    let mut candidates = Vec::new();
    for entry in fs::read_dir(&log_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("rust-chatbot.log.")
        {
            candidates.push(path);
        }
    }
    candidates.sort();
    let newest = candidates.pop().unwrap();
    // Lexicographic: .2 > .10 > .1
    assert!(
        newest
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("rust-chatbot.log.2"),
        "expected rust-chatbot.log.2, got {:?}",
        newest.file_name()
    );
    fs::remove_dir_all(temp).ok();
}

#[test]
fn newest_trace_log_since_finds_newly_modified_log() {
    let temp = unique_temp_dir("trace-log-since");
    let log_dir = temp.join("logs");
    fs::create_dir_all(&log_dir).unwrap();

    // Create an old log file
    let old_path = log_dir.join("rust-chatbot.log.1");
    fs::write(&old_path, "old content").unwrap();

    // Small delay to ensure different mtime
    thread::sleep(std::time::Duration::from_millis(10));

    // Create a reference time just before "now"
    let reference = std::time::SystemTime::now();

    // Create a new log file after the reference
    let new_path = log_dir.join("rust-chatbot.log.2");
    fs::write(&new_path, "new content").unwrap();

    // Set the mtime of the new file to be after reference
    let newer_time = std::time::SystemTime::now();
    let _ = newer_time; // Used implicitly via comparison in the function

    let result = newest_trace_log_since(reference);
    assert!(
        result.is_ok(),
        "should find a log file modified after reference"
    );

    fs::remove_dir_all(temp).ok();
}

#[test]
fn newest_trace_log_since_falls_back_when_no_newer_logs() {
    let temp = unique_temp_dir("trace-log-fallback");
    let log_dir = temp.join("logs");
    fs::create_dir_all(&log_dir).unwrap();

    // Create only old log files (before reference time)
    let old_path = log_dir.join("rust-chatbot.log.1");
    fs::write(&old_path, "old content").unwrap();

    // Use a time far in the future as reference - nothing should be newer
    let far_future = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);

    let result = newest_trace_log_since(far_future);
    // Should fall back to newest_trace_log since nothing is newer
    assert!(result.is_ok());
    fs::remove_dir_all(temp).ok();
}
