# Rust Design: Generic Desktop UI Harness

## Status

This document describes the Rust architecture for `auto-ui`.

The migration from the extracted single-purpose tooling into a Rust workspace is
implemented. The repo now contains:

- a CLI crate
- a shared core crate
- a report/artifact crate
- an X11 driver crate
- a `rust-chatbot` adapter crate
- a `gpui-component-testing` adapter crate

The remaining work is no longer the migration itself. It is the follow-on
hardening and extension work documented in [REMAINING.md](/home/m/git/auto-ui/REMAINING.md).

## Problem

The current repo contains Python scripts tailored to one app:

- `rust-chatbot` uses a managed desktop app with:
  - release binaries such as `chatbot-ctl`
  - session metadata in provider-owned JSON files
  - trace logs under the local state directory
  - window-level automation via `xdotool`, `wmctrl`, and ImageMagick
- `gpui-component-testing` already supports startup-driven automation:
  - env vars select the scenario/example
  - the app auto-runs scroll or trace behavior on startup
  - artifacts are emitted as CSV/log/markdown without pointer input

The extracted repo is useful, but it is still a `rust-chatbot` extraction rather
than a general-purpose automation tool.

## Goals

- Rebuild the tool in Rust.
- Treat `rust-chatbot` as the first adapter, not the product.
- Drive both `rust-chatbot` and `/home/m/git/gpui-component-testing`.
- Support two execution styles:
  - interactive window automation
  - startup-driven autonomous scenarios
- Define a stable artifact model for reports, images, traces, and metrics.
- Keep builds outside the core automation loop when possible.
- Make the CLI and config generic enough to add more desktop targets later.

## Non-Goals

- A browser-first automation framework.
- Full OCR as a first milestone.
- Windows support in the current phase.
- Wayland support in the current phase.
- Replacing app-owned benchmark logic with generic automation when the app can
  already self-drive more reliably.
- Auto-compiling arbitrary repos by default.
- Headless virtual display on macOS (`--headless` is Linux-only).

## Design Principles

1. Prefer startup-driven automation over synthetic pointer input when the target
   app supports it.
2. Separate generic harness logic from app-specific launch and trace behavior.
3. Keep artifacts structured and machine-readable first, human-readable second.
4. Make action drivers pluggable so the system is not permanently tied to
   `xdotool`.
5. Avoid hard-coded knowledge like PingLine session lists in the core.
6. Strive to be minimally conflicting with a desktop user's active workflow.

## Desktop Coexistence Policy

The harness should behave like a background tool by default, not like the
primary owner of the desktop session.

Defaults:

- Prefer startup-driven automation or app-owned self-test hooks over pointer
  automation.
- Do not move the mouse pointer unless a scenario explicitly opts in and there
  is no reliable keyboard, launch, or accessibility-driven alternative.
- Start target windows without leaving them in the foreground when the scenario
  does not require live interaction. Launch, attach, then lower them.
- Default `keep_front` to `false` and treat foreground execution as an explicit
  opt-in.
- Send input only to the verified target window, never to whichever window
  currently has user focus.
- Use app-scoped key delivery instead of global shortcuts whenever the driver
  can target a specific window.
- Restore temporary window state when practical after capture steps, especially
  if the harness changed focus, z-order, or geometry.

Avoid:

- stealing focus for longer than the action strictly requires
- switching workspaces or virtual desktops unless the scenario explicitly asks
  for it
- sending OS-level shortcuts that may affect unrelated apps or the window
  manager
- using the clipboard, primary selection, or drag-and-drop as a transport
  mechanism when another channel is available
- repositioning unrelated windows
- depending on pointer hover state as the primary trigger when keyboard or
  startup hooks can express the same action

Recommended safeguards:

- verify the target window id before every key-send or resize action
- lower the window again after any required foreground-only step
- expose opt-in flags for intrusive behavior instead of making it implicit
- log when the harness had to take foreground control so runs can be audited

## Target Launch Contract

The harness cannot guarantee background-first window creation by itself on X11.
The window manager decides focus when a new window is first mapped.

To let target apps cooperate with the desktop-coexistence policy, `auto-ui`
sets `AUTO_UI_LAUNCH_BACKGROUND=1` on launches that should avoid taking
foreground control.

Contract:

- if `AUTO_UI_LAUNCH_BACKGROUND=1`, the target should create or map its first
  automation window without requesting focus when the toolkit/backend supports
  that behavior
- if the stack cannot honor the request, the target may ignore it and launch
  normally
- the harness still lowers the window after map as a fallback, so the env var
  is a best-effort request rather than a hard guarantee

## High-Level Architecture

The repo is now a Cargo workspace with the planned split in place.

```text
auto-ui/
├── Cargo.toml
├── crates/
│   ├── auto-ui-cli/
│   ├── auto-ui-core/
│   ├── auto-ui-artifacts/
│   ├── auto-ui-driver-x11/
│   ├── auto-ui-adapter-rust-chatbot/
│   └── auto-ui-adapter-gpui/
├── examples/
├── DESIGN.md
└── REMAINING.md
```

## Core Components

### `auto-ui-core`

Owns the execution model:

- scenario parsing
- run orchestration
- adapter lifecycle
- event stream aggregation
- failure handling
- timing and retry policy

Key types:

```rust
pub enum ExecutionMode {
    StartupDriven,     // Harness launches target, waits for completion
    InteractiveWindow,  // Harness manages interactive window
    Hybrid,            // Startup launch + interactive window management
}

pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Error,
}

pub struct RunRequest {
    pub target: String,           // Target identifier (e.g., "rust_chatbot")
    pub scenario: String,        // Scenario name (e.g., "debug", "scroll_matrix")
    pub execution_mode: ExecutionMode,
    pub output_dir: Option<String>,
    pub config: serde_json::Value,  // Additional scenario-specific config
}

pub struct RunResult {
    pub status: RunStatus,
    pub output_dir: std::path::PathBuf,
    pub report_path: std::path::PathBuf,
    pub error: Option<String>,   // Present only if status is Error
}
```

### `auto-ui-artifacts`

Defines the stable on-disk schema for:

- screenshots
- crops
- JSON reports
- trace bundles
- imported external artifacts
- summaries

The artifact model needs to work for both apps:

- `rust-chatbot`: screenshots + parsed trace fields + code block traces
- `gpui-component-testing`: imported CSV/log/summary outputs produced by the
  launched example

### `auto-ui-driver-x11`

Owns generic Linux desktop interaction:

- find window by PID or title
- resize window
- activate/lower window
- send key input
- capture screenshots
- query geometry

### `auto-ui-adapter-rust-chatbot`

Owns `rust-chatbot` specifics:

- reading provider session metadata
- launching provider-specific app instances
- parsing `rust-chatbot` trace logs
- scenario implementations for:
  - `debug`
  - `header_debug`
  - `prompt_debug`

### `auto-ui-adapter-gpui`

Owns `gpui-component-testing` specifics:

- launching an example binary with env vars
- importing app-generated artifacts into the stable report format
- scenario implementations for:
  - `scroll_matrix`
  - `scrollbar_trace`
  - `conversation_paint`

### `auto-ui-cli`

Owns:

- clap command parsing
- target/scenario discovery commands
- output directory setup
- error rendering

## Scenario Model

Scenarios are config-driven.

```toml
target = "rust_chatbot"
scenario = "debug"

[app]
root = "/path/to/rust-chatbot"
provider = "codex"

[window]
widths = [520, 900, 1000]
height = 900
```

Design expectations:

- the scenario format should stay generic at the top level
- app-specific config should remain nested and explicit
- adapters validate only the scenarios they own

## Execution Flow

### Interactive window flow

Used primarily by `rust-chatbot`:

1. Resolve target root and binaries.
2. Resolve provider/session metadata.
3. Launch or attach to a provider window.
4. Resize/focus/lower as required.
5. Capture screenshots and parse traces.
6. Write a stable JSON report.

### Startup-driven flow

Used primarily by `gpui-component-testing`:

1. Resolve target root and scenario binary.
2. Launch with scenario-specific env vars.
3. Wait for completion or artifact emission.
4. Import emitted CSV/log/images.
5. Write a stable JSON report.

## Report Contract

Every run writes an output directory containing:

- `report.json`
- screenshots or imported artifacts
- optional crops or enhanced images
- adapter-specific raw outputs when useful

The report shape is intentionally stable at the top level and flexible under
adapter-owned detail sections.

## Extension Model

Adding a new target should require:

1. a new adapter crate
2. target registration in the CLI
3. scenario docs/examples
4. zero or minimal changes to the X11 driver and report schema crates

## Rust-Chatbot Adapter Design

### Required capabilities

- Resolve the target repo root.
- Resolve `target/release/chatbot-ctl` and `target/release/rust-chatbot`.
- Read provider metadata from `~/.codex-desktop`, `~/.claude-desktop`,
  `~/.gemini-desktop`, `~/.gemini-forge-desktop`, or
  `~/.minimax-forge-desktop`.
- Launch a session with:
  - `AUTO_UI_LAUNCH_BACKGROUND=1`
  - `RUST_CHATBOT_AUTO_UI_DEBUG=1`
  - optional `RUST_CHATBOT_START_SESSION_ID`
- Parse the latest `rust-chatbot.log.*` file for:
  - `ui_auto_debug`
  - `ui_auto_debug_code_block`
  - `ai_response_end`
  - markdown render timing traces

### Output shape

The adapter should emit:

- session metadata summary
- requested widths or prompt info
- screenshots/crops
- trace field maps
- any provider session id discovered from session JSON

## GPUI Adapter Design

### Required capabilities

- Resolve the target repo root.
- Resolve or infer the example binary path.
- Launch with scenario-specific env vars.
- Import app-generated artifacts without reparsing them into an unrelated shape.

## Failure Model

Errors should be explicit about:

- missing app roots
- missing release binaries
- missing session metadata
- missing windows
- missing logs
- malformed scenario config

The harness should fail closed rather than silently picking the wrong target.

## Test Strategy

The repo should cover three layers:

1. Unit tests for config parsing, path resolution, and report shaping.
2. Adapter tests for scenario validation and metadata parsing.
3. Optional live smoke tests gated by env vars for real desktop sessions.

## Cross-Platform Driver Architecture

The `WindowDriver` trait in `auto-ui-core` is the portability boundary. Each platform provides its own implementation:

- **Linux/X11**: `X11WindowDriver` uses `xdotool`, `wmctrl`, and ImageMagick `import`.
- **macOS**: `MacOsWindowDriver` uses CoreGraphics (`CGWindowListCopyWindowInfo`, `CGWindowListCreateImage`), `CGEventPost` for input, and AppleScript for geometry/activation.
- **Image processing**: `crop_metric` and `image_size` are platform-agnostic, implemented in `auto-ui-core` with the Rust `image` crate (ImageMagick eliminated).

See [docs/macos-support-design.md](/home/m/git/auto-ui/docs/macos-support-design.md) for the full macOS implementation design.

## Future Work

Follow-on items are tracked in [REMAINING.md](/home/m/git/auto-ui/REMAINING.md).
