# Troubleshooting Guide

This guide covers common issues when running `auto-ui` and how to resolve them.

## Missing Display

`auto-ui` requires a Linux desktop session with `DISPLAY` set.

**Error:** `could not create home directory` or display-related failures

**Solution:**
- Ensure `DISPLAY` is set: `echo $DISPLAY`
- If running headless, use `--headless` flag to create a private Xvfb display
- For CI/headless: `auto-ui run --config scenario.toml --headless --geometry 1280x800x24`

## Missing Tools

`auto-ui` requires: `xdotool`, `wmctrl`, ImageMagick (`import`, `convert`, `identify`).

**Error:** `failed to find/install tool`
**Error:** `xdotool not found`
**Error:** `import: command not found`

**Solution:**
```bash
# Debian/Ubuntu
sudo apt install xdotool wmctrl imagemagick

# Fedora
sudo dnf install xdotool wmctrl ImageMagick
```

## Missing Binaries

`auto-ui` does not build target apps. You need release binaries already built.

**Error:** `chatbot-ctl not found at <path>/target/release/chatbot-ctl`
**Error:** `rust-chatbot not found at <path>/target/release/rust-chatbot`

**Solution:**
1. Build the target app through its own build system
2. Point `auto-ui` at the checkout with `--app-root` or env var:
   - `RUST_CHATBOT_APP_ROOT` for rust-chatbot
   - `GPUI_COMPONENT_TESTING_ROOT` for GPUI

## Trace Timeouts

`auto-ui` waits for trace output from the target app to determine when UI is ready.

**Error:** `No ui_auto_debug trace observed for session`
**Error:** `Timed out waiting for ai_response_end`

**Possible causes:**
1. The session was not started with `RUST_CHATBOT_AUTO_UI_DEBUG=1`
2. Trace logging is not enabled in the target app
3. Trace output is being written to a different log file

**Solution:**
- Ensure the target app is launched with `RUST_CHATBOT_AUTO_UI_DEBUG=1` env var
- Check log file permissions in `~/.local/state/rust-chatbot/`
- Increase `trace_timeout` in your scenario config (default: 5 seconds)

## Stale Logs

When multiple `auto-ui` runs happen quickly, the wrong trace log may be selected.

**Problem:** `auto-ui` selects an old log file instead of the one from the current run.

**Solution:**
`auto-ui` now uses `newest_trace_log_since()` to find logs modified after launch. If you encounter stale log issues:
- Ensure the log directory (`~/.local/state/rust-chatbot/`) is writable
- Check that log rotation is not moving files unexpectedly

## Artifact Failures

Reports include artifact references that must exist on disk.

**Error:** `report validation failed: artifact not found`

**Solution:**
- Check that the reported artifact path exists
- Verify the command that was supposed to create it actually ran
- Look at the `progress.log` for the actual command output

## Window Not Found

**Error:** `No window matching 'Rust Chatbot' was found`
**Error:** `Could not find a new window matching 'Rust Chatbot' after launch`

**Solution:**
- Use `--no-launch` if the window is already open
- Set `launch_if_missing = true` to auto-launch (default for most scenarios)
- Check that the window title matches the expected pattern: `<provider> - Rust Chatbot`

## Session Resolution Failures

**Error:** `No visible sessions with messages were found`
**Error:** `Session payload for <id> is missing an id`

**Solution:**
- Ensure the provider has active sessions: `AUTO_UI_TEST_RUST_CHATBOT_SESSION_ID`
- Check `sessions.json` is readable in the provider data directory
- Verify `message_count > 0` for visible sessions

## Screenshot Failures

**Error:** `failed to capture screenshot`

**Solution:**
- ImageMagick `import` command must be available
- The window must be visible (not minimized)
- DISPLAY must be accessible

## Build Hint Display

When `auto-ui` detects binaries are not built:

**Message:** `build_hint: run 'cargo build --release' in <target>`

This is informational only - `auto-ui` does not automatically build targets.