# auto-ui

Rust desktop UI automation helpers for desktop apps that can be driven either
through live window interaction or startup-driven scenarios.

This repo currently supports both `rust-chatbot` and
`gpui-component-testing`.

## What it contains

- `Cargo.toml`: workspace root
- `crates/auto-ui-cli`: CLI implementation
- `crates/auto-ui-core`: shared command, path, and scenario helpers
- `crates/auto-ui-artifacts`: report and artifact schema
- `crates/auto-ui-driver-x11`: Linux/X11 window and screenshot driver
- `crates/auto-ui-adapter-rust-chatbot`: rust-chatbot adapter
- `crates/auto-ui-adapter-gpui`: gpui-component-testing adapter

## Requirements

- Linux desktop session with `DISPLAY` set
- Rust toolchain installed locally
- `xdotool`
- `wmctrl`
- ImageMagick tools: `import`, `convert`, `identify`
- A separate target app checkout with release binaries already built

This repo does not build target apps itself. Build the target app through your
lightweight build path first, then point `auto-ui` at that checkout. Build this
repo through the same lightweight path before running the binary.

## Target roots

- `rust-chatbot`
  1. `--app-root`
  2. `RUST_CHATBOT_APP_ROOT`
  3. `RUST_CHATBOT_ROOT`
  4. sibling checkout at `/home/m/git/rust-chatbot`
- `gpui-component-testing`
  1. app root in the scenario file
  2. `GPUI_COMPONENT_TESTING_ROOT`
  3. `AUTO_UI_GPUI_APP_ROOT`
  4. sibling checkout at `/home/m/git/gpui-component-testing`

## Commands

- `debug`: rust-chatbot width scan across sessions
- `header-debug`: rust-chatbot single-session header capture flow
- `run --config <file.toml>`: generic scenario runner
- `report-schema`: print the stable `report.json` JSON schema
- `targets`: list supported targets
- `scenarios [--target <name>]`: list supported scenarios

## Examples

See [examples/README.md](/home/m/git/auto-ui/examples/README.md) for a small
set of copyable example invocations.

```bash
target/debug/auto-ui debug --provider codex
```

```bash
target/debug/auto-ui debug \
  --app-root /home/m/git/rust-chatbot \
  --provider codex \
  --session-id 64ee1661-b53f-4134-8855-cc2c25a06ddd
```

```bash
target/debug/auto-ui header-debug \
  --app-root /home/m/git/rust-chatbot \
  --provider codex \
  --session-name "rcc-header"
```

```bash
target/debug/auto-ui run --config examples/gpui-scroll-matrix.toml
```

```bash
target/debug/auto-ui run --config examples/gpui-conversation-paint.toml
```

```bash
target/debug/auto-ui report-schema > report.schema.json
```

Artifacts land under `tmp/` in this repo unless `--output-dir` is provided.

## Report Schema

`report.json` now has an explicit machine-readable schema surface.

- Use `target/debug/auto-ui report-schema` to print the current JSON schema.
- See [REPORT_SCHEMA.md](/home/m/git/auto-ui/REPORT_SCHEMA.md) for the stable top-level contract and field notes.

## Live Smoke Tests

Ignored live smoke tests exist for the adapters. They require a real desktop
session, built target app binaries, and explicit env vars.

- Set `AUTO_UI_RUN_LIVE_TESTS=1` to opt in.
- Rust Chatbot tests also require `AUTO_UI_TEST_RUST_CHATBOT_ROOT` and
  `AUTO_UI_TEST_RUST_CHATBOT_SESSION_ID`.
- GPUI tests require `AUTO_UI_TEST_GPUI_ROOT` and cover `scroll_matrix`,
  `scrollbar_trace`, and `conversation_paint`.

## Desktop Behavior

The automation should strive to be minimally conflicting with a desktop user's
active workflow.

- Prefer startup-driven automation over mouse-driven interaction when the app
  supports it.
- Do not move the mouse pointer unless a scenario explicitly opts in and there
  is no reliable alternative.
- Do not leave automation windows in the foreground by default. Launch them,
  resize or attach as needed, then lower them again.
- Treat `keep_front = true` style behavior as an explicit opt-in.
- When reusing an existing window, restore its prior geometry after the run
  when practical.
- Send keys only to the verified target window, not to the user's currently
  focused app.
- Avoid switching workspaces, sending global shortcuts, or using the clipboard
  when another mechanism is available.

Target launch contract:

- When `auto-ui` launches a target in background-friendly mode, it sets
  `AUTO_UI_LAUNCH_BACKGROUND=1`.
- Target apps that understand this contract should create or map their first
  automation window without requesting focus when the toolkit/backend supports
  it.
- Targets may ignore the request on unsupported stacks. `auto-ui` still lowers
  the window after map as a fallback.

## Status

The workspace migration is implemented. The remaining work is refinement and
extension rather than the original architectural split.

See:

- [DESIGN.md](/home/m/git/auto-ui/DESIGN.md) for the implemented architecture and target shape
- [REMAINING.md](/home/m/git/auto-ui/REMAINING.md) for the post-migration backlog
