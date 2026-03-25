# `auto-ui --help`

These snapshots were taken from the current built binary in this repo.

## Top-level help

```text
Generic desktop UI automation helpers

Usage: auto-ui <COMMAND>

Commands:
  debug
  header-debug
  run
  targets
  scenarios
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `auto-ui debug --help`

```text
Usage: auto-ui debug [OPTIONS]

Options:
      --app-root <APP_ROOT>
      --provider <PROVIDER>
          [default: codex] [possible values: claude, codex, gemini]
      --instance <INSTANCE>
      --widths <WIDTHS>
          [default: 520,900,1000]
      --height <HEIGHT>
          [default: 900]
      --max-sessions <MAX_SESSIONS>
          [default: 8]
      --session-id <SESSION_ID>
      --include-hidden
      --no-launch
          Reuse an existing matching window instead of launching a fresh one.
      --window-timeout <WINDOW_TIMEOUT>
          [default: 15]
      --trace-timeout <TRACE_TIMEOUT>
          [default: 5]
      --settle <SETTLE>
          [default: 0.7]
      --output-dir <OUTPUT_DIR>
      --keep-front
          Do not lower launched automation windows behind other windows.
  -h, --help
          Print help
```

## `auto-ui header-debug --help`

```text
Usage: auto-ui header-debug [OPTIONS]

Options:
      --app-root <APP_ROOT>
      --provider <PROVIDER>              [default: codex] [possible values: claude, codex, gemini]
      --instance <INSTANCE>
      --session-id <SESSION_ID>
      --session-name <SESSION_NAME>
      --include-hidden
      --widths <WIDTHS>                  [default: 520,900,1000]
      --height <HEIGHT>                  [default: 900]
      --header-height <HEADER_HEIGHT>    [default: 140]
      --window-timeout <WINDOW_TIMEOUT>  [default: 15]
      --trace-timeout <TRACE_TIMEOUT>    [default: 8]
      --settle <SETTLE>                  [default: 0.8]
      --output-dir <OUTPUT_DIR>
      --keep-front                       Do not lower the launched window behind other windows.
  -h, --help                             Print help
```

## `auto-ui run --help`

```text
Usage: auto-ui run [OPTIONS] --config <CONFIG>

Options:
      --config <CONFIG>
      --target <TARGET>
      --scenario <SCENARIO>
      --output-dir <OUTPUT_DIR>
  -h, --help                     Print help
```

## Discovery helpers

```bash
cd /home/m/git/auto-ui
./target/debug/auto-ui targets
./target/debug/auto-ui scenarios
./target/debug/auto-ui scenarios --target rust_chatbot
./target/debug/auto-ui scenarios --target gpui_component_testing
```

Current `targets` output:

```text
rust_chatbot
gpui_component_testing
```

Current `scenarios` output:

```text
rust_chatbot:
  debug
  header_debug
gpui_component_testing:
  scroll_matrix
  scrollbar_trace
```
