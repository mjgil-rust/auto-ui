# `auto-ui --help`

These snapshots were taken from the current built binary in this repo.

## Top-level help

```text
Generic desktop UI automation helpers

Usage: auto-ui <COMMAND>

Commands:
  debug             Run rust-chatbot debug session with window measurements
  header-debug      Run rust-chatbot header debug with screenshot and crop metrics
  run               Run a generic scenario from a TOML config file
  report-schema     Print the JSON schema for report files
  targets           List available automation targets (rust_chatbot, gpui_component_testing)
  scenarios         List scenarios for a target (use --target to filter)
  scenario-schema   Print the JSON schema for scenario configuration files
  validate          Validate a TOML scenario configuration file
  inspect           Inspect an existing report.json file
  help              Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `auto-ui debug --help`

```text
Run rust-chatbot debug session with window measurements

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
Run rust-chatbot header debug with screenshot and crop metrics

Usage: auto-ui header-debug [OPTIONS]

Options:
      --app-root <APP_ROOT>              Path to rust-chatbot root (overrides AUTO_UI_APP_ROOT env var)
      --provider <PROVIDER>              [default: codex] [possible values: claude, codex, gemini]
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
      --output-dir <OUTPUT_DIR>          Output directory for artifacts
      --keep-front                       Do not lower the launched window behind other windows.
  -h, --help                             Print help
```

## `auto-ui run --help`

```text
Run a generic scenario from a TOML config file

Usage: auto-ui run [OPTIONS] --config <CONFIG>

Options:
      --config <CONFIG>              Path to TOML scenario configuration file
      --target <TARGET>              Target to run (rust_chatbot or gpui_component_testing). Overrides the target field in the TOML file.
      --scenario <SCENARIO>          Scenario to run within the target. Overrides the scenario field in the TOML file.
      --output-dir <OUTPUT_DIR>      Output directory for artifacts and reports. Defaults to a unique timestamped directory.
      --headless                     Run on a private Xvfb display with openbox so no windows appear on the user's real screen. Requires Xvfb and openbox to be installed
      --geometry <GEOMETRY>          Xvfb screen geometry (default: 1280x800x24). Only used with --headless [default: 1280x800x24]
  -h, --help                         Print help
```

## `auto-ui inspect --help`

```text
Inspect an existing report.json file

Usage: auto-ui inspect --report <REPORT>

Options:
      --report <REPORT>  Path to report.json file to inspect
  -h, --help             Print help
```

## `auto-ui scenario-schema --help`

```text
Print the JSON schema for scenario configuration files

Usage: auto-ui scenario-schema [OPTIONS]

Options:
      --target <TARGET>  Target to show schemas for (rust_chatbot or gpui_component_testing). If omitted, shows schemas for all targets.
  -h, --help            Print help
```

## `auto-ui validate --help`

```text
Validate a TOML scenario configuration file

Usage: auto-ui validate --config <CONFIG>

Options:
      --config <CONFIG>  Path to TOML scenario configuration file to validate
  -h, --help             Print help
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
Available scenarios:
  rust_chatbot (3 scenarios)
    debug
    header_debug
    prompt_debug
  gpui_component_testing (3 scenarios)
    scroll_matrix
    scrollbar_trace
    conversation_paint
```

Filtered `scenarios --target rust_chatbot`:

```text
rust_chatbot scenarios:
  debug
  header_debug
  prompt_debug
```

Filtered `scenarios --target gpui_component_testing`:

```text
gpui_component_testing scenarios:
  scroll_matrix
  scrollbar_trace
  conversation_paint
```
