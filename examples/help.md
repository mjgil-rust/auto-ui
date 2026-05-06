# Current Help Output

## `targets`

```text
Supported targets:
  rust_chatbot
  gpui_component_testing
```

## `scenarios`

```text
Supported scenarios:
  rust_chatbot:
    debug
    header_debug
    prompt_debug
  gpui_component_testing:
    scroll_matrix
    scrollbar_trace
    conversation_paint
```

## `debug --help`

```text
Usage: auto-ui debug [OPTIONS]

Options:
      --app-root <APP_ROOT>              Path to rust-chatbot root (overrides AUTO_UI_APP_ROOT env var)
      --provider <PROVIDER>              [default: codex] [possible values: claude, codex, gemini, geminiforge, minimaxforge]
      --instance <INSTANCE>              Provider instance identifier
      --widths <WIDTHS>                  [default: 520,900,1000]
      --height <HEIGHT>                  [default: 900]
      --max-sessions <MAX_SESSIONS>      [default: 8]
      --session-id <SESSION_ID>          Start in a specific session by ID
      --include-hidden                   Include hidden sessions in session list
      --launch                           Launch a fresh app window with RUST_CHATBOT_AUTO_UI_DEBUG=1 and use that window.
      --no-launch                        Reuse an existing matching window instead of launching a fresh one.
      --window-timeout <WINDOW_TIMEOUT>  [default: 15]
      --trace-timeout <TRACE_TIMEOUT>    [default: 5]
      --settle <SETTLE>                  [default: 0.7]
      --output-dir <OUTPUT_DIR>          Override the run output directory
      --keep-front                       Do not lower launched automation windows behind other windows
  -h, --help                             Print help
```

## `header-debug --help`

```text
Usage: auto-ui header-debug [OPTIONS]

Options:
      --app-root <APP_ROOT>              Path to rust-chatbot root (overrides AUTO_UI_APP_ROOT env var)
      --provider <PROVIDER>              [default: codex] [possible values: claude, codex, gemini, geminiforge, minimaxforge]
      --instance <INSTANCE>              Provider instance identifier
      --session-id <SESSION_ID>          Start in a specific session by ID
      --session-name <SESSION_NAME>      Start in a specific session by name (for header debug)
      --include-hidden                   Include hidden sessions in session list
      --widths <WIDTHS>                  [default: 520,900,1000]
      --height <HEIGHT>                  [default: 900]
      --header-height <HEADER_HEIGHT>    [default: 140]
      --window-timeout <WINDOW_TIMEOUT>  [default: 15]
      --trace-timeout <TRACE_TIMEOUT>    [default: 8]
      --settle <SETTLE>                  [default: 0.8]
      --output-dir <OUTPUT_DIR>          Override the run output directory
      --keep-front                       Do not lower the launched window behind other windows
  -h, --help                             Print help
```
