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
- Cross-platform parity in v1.
- Replacing app-owned benchmark logic with generic automation when the app can
  already self-drive more reliably.
- Auto-compiling arbitrary repos by default.

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

This crate should wrap `xdotool`, `wmctrl`, and ImageMagick initially, but hide
them behind traits so a later backend can replace the shell tools.

### `auto-ui-adapter-rust-chatbot`

App-specific logic for:

- resolving `chatbot-ctl`
- reading provider session metadata
- launching targeted sessions
- locating the current trace log
- parsing `ui_auto_debug` and `ui_auto_debug_code_block` events
- mapping session-oriented scenarios into launch inputs

### `auto-ui-adapter-gpui`

App-specific logic for:

- choosing an example binary or launch command
- injecting scenario env vars
- waiting for the app to auto-complete and exit
- importing generated CSV/log/markdown artifacts
- optionally attaching to the created window when screenshots are requested

This adapter is the reason the system must support startup-driven scenarios as a
first-class mode, not as an edge case.

## Adapter Interface

Each target app should implement a common adapter trait.

```rust
pub trait TargetAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn discover_scenarios(&self, ctx: &AdapterContext) -> anyhow::Result<Vec<ScenarioRef>>;
    fn prepare(&self, ctx: &AdapterContext, scenario: &ScenarioSpec) -> anyhow::Result<PreparedRun>;
    fn launch(&self, ctx: &AdapterContext, prepared: &PreparedRun) -> anyhow::Result<LaunchedRun>;
    fn collect(&self, ctx: &AdapterContext, run: &LaunchedRun) -> anyhow::Result<CollectedData>;
    fn stop(&self, ctx: &AdapterContext, run: &LaunchedRun) -> anyhow::Result<()>;
}
```

Supporting types:

```rust
pub struct PreparedRun {
    pub strategy: LaunchStrategy,
    pub expected_window: Option<WindowSelector>,
    pub trace_source: Option<TraceSource>,
    pub import_sources: Vec<ImportSource>,
    pub env: std::collections::BTreeMap<String, String>,
    pub args: Vec<String>,
}

pub enum LaunchStrategy {
    ExistingWindow(WindowSelector),
    ManagedProcess(CommandSpec),
    AutonomousProcess(CommandSpec),
}
```

The important point is that adapters do not expose raw window-manager behavior.
They describe what kind of run the harness should execute.

## Execution Strategies

The harness should support three concrete strategies.

### 1. Startup-Driven

Use when the app can drive itself after launch.

Flow:

1. adapter prepares launch command and env
2. harness launches the process
3. app performs its own scripted behavior
4. harness waits for completion or timeout
5. adapter imports artifacts and optional logs

Best fit:

- `gpui-component-testing`
- `rust-chatbot` runs that can be launched directly into a target session and
  emit traces without manual selection

### 2. Interactive Window

Use when the harness must manipulate a live window.

Flow:

1. attach to existing window or process
2. resize, focus, send keys, optionally click
3. capture traces and screenshots between actions

Best fit:

- attaching to an already-open `rust-chatbot` window
- later generic desktop targets with no startup automation hooks

### 3. Hybrid

Launch the app in a known startup state, then apply a small amount of generic
window automation.

Best fit:

- `rust-chatbot`: launch directly into a specific session, then resize and
  capture

## Scenario Model

The current Python scripts hard-code some behavior that should move into a
declarative scenario format.

Suggested format: TOML.

Example:

```toml
target = "rust_chatbot"
scenario = "session-width-scan"
mode = "hybrid"

[app]
root = "/home/m/git/rust-chatbot"
provider = "codex"
session_id = "64ee1661-b53f-4134-8855-cc2c25a06ddd"

[window]
widths = [520, 900, 1000]
height = 900
keep_front = false

[capture]
full_window = true
header_crop = true
trace_bundle = true
```

Example for `gpui-component-testing`:

```toml
target = "gpui_component_testing"
scenario = "scroll-matrix"
mode = "startup-driven"

[app]
root = "/home/m/git/gpui-component-testing"
example = "llm_chat_story_style_bench_demo"

[env]
BENCH_AUTO_SCROLL = "1"
BENCH_SCROLL_WARMUP_MS = "1500"
BENCH_SCROLL_TICK_MS = "16"
BENCH_SCROLL_STEP_PX = "40"

[capture]
import_csv = true
import_stderr = true
window_screenshot = false
```

## Rust-Chatbot Adapter Design

### Required capabilities

- Resolve the target repo root.
- Resolve `target/release/chatbot-ctl` and `target/release/rust-chatbot`.
- Read provider metadata from `~/.codex-desktop`, `~/.claude-desktop`, or
  `~/.gemini-desktop`.
- Launch a session with:
  - `AUTO_UI_LAUNCH_BACKGROUND=1`
  - `RUST_CHATBOT_AUTO_UI_DEBUG=1`
  - optional `RUST_CHATBOT_START_SESSION_ID`
- Parse the latest `rust-chatbot.log.*` file for:
  - `ui_auto_debug`
  - `ui_auto_debug_code_block`
- Capture screenshots and derived crops.

### Preferred mode

`Hybrid`.

The adapter should avoid selecting sessions by pointer interaction when it can
launch directly into the desired session.

### Special data sources

- provider sessions metadata
- provider session JSON files
- XDG/local state trace logs

## GPUI Component Testing Adapter Design

### What the repo already gives us

The existing scripts are not a generic automation framework, but they confirm a
strong pattern:

- launch an example binary
- inject env vars to select behavior
- let the app auto-run
- wait for process exit
- read generated artifacts

That is exactly the behavior the Rust harness should model directly.

### Required capabilities

- Resolve the target repo root.
- Resolve a prebuilt example binary or a configured launch command.
- Inject env vars such as:
  - `AUTO_UI_LAUNCH_BACKGROUND`
  - `BENCH_AUTO_SCROLL`
  - `BENCH_SCROLL_WARMUP_MS`
  - `BENCH_SCROLL_TICK_MS`
  - `BENCH_SCROLL_STEP_PX`
  - `BENCH_WINDOW_TITLE`
  - `SCROLLBAR_DEMO_*`
  - `GPUI_COMPONENT_SCROLLBAR_TRACE`
- Optionally import files from the artifact directory after the process exits.
- Optionally attach to the launched window for screenshot capture when the
  scenario requests visual output.

### Preferred mode

`StartupDriven`.

The adapter should not default to mouse interaction for this repo. The app
already knows how to benchmark and trace itself more reliably than an external
pointer script.

### Artifact support

The adapter must be able to ingest:

- `.csv`
- `.log`
- `.md`
- screenshots taken externally by the harness

## Driver Layer

The initial implementation should keep Linux support only.

```rust
pub trait WindowDriver: Send + Sync {
    fn find_window(&self, selector: &WindowSelector) -> anyhow::Result<Option<WindowHandle>>;
    fn resize(&self, window: &WindowHandle, width: u32, height: u32) -> anyhow::Result<WindowGeometry>;
    fn activate(&self, window: &WindowHandle) -> anyhow::Result<()>;
    fn lower(&self, window: &WindowHandle) -> anyhow::Result<()>;
    fn send_keys(&self, window: &WindowHandle, keys: &[KeyChord]) -> anyhow::Result<()>;
    fn screenshot(&self, window: &WindowHandle, path: &std::path::Path) -> anyhow::Result<()>;
    fn geometry(&self, window: &WindowHandle) -> anyhow::Result<WindowGeometry>;
}
```

Later backends:

- AT-SPI/accessibility
- DevTools-controlled browser targets
- Wayland-specific drivers

## Artifact Schema

The report shape is now explicit and exported by the CLI with:

```text
auto-ui report-schema
```

Stable top-level shape:

```json
{
  "schema_version": "1",
  "tool_version": "0.1.0",
  "target": "rust_chatbot",
  "scenario": "session-width-scan",
  "mode": "hybrid",
  "app_root": "/home/m/git/rust-chatbot",
  "run_id": "20260325-214500-abc123",
  "artifacts": [],
  "measurements": [],
  "events": [],
  "status": "ok"
}
```

Important constraint: imported target-generated artifacts and harness-generated
artifacts should look the same at the report layer. Adapter-specific expansion
should stay nested under `details`, `measurements[]`, `events[]`, or artifact
`metadata` unless it is clearly shared across targets.

## CLI Design

The CLI should be explicit and generic.

```text
auto-ui run --target rust_chatbot --scenario session-width-scan --config run.toml
auto-ui run --target gpui_component_testing --scenario scroll-matrix --config run.toml
auto-ui targets
auto-ui scenarios --target rust_chatbot
auto-ui report-schema
```

Suggested command structure:

- `run`
- `targets`
- `scenarios`
- `report-schema`

## Build and Launch Policy

The harness should not compile target apps by default.

Reason:

- build orchestration is separate from UI automation
- some repos already have external build rules
- startup/debug loops are faster and more predictable with prebuilt binaries

Recommended model:

- core assumes binaries already exist
- adapters may optionally expose a `build_hint`
- a later integration may call an external lightweight build service before the
  run begins

This keeps the automation system reusable across repos with different build
pipelines.

## Configuration Resolution

Resolution order should be:

1. CLI flag
2. scenario file
3. environment variable
4. adapter default

This is especially important for:

- app root
- provider
- example name
- artifact directory
- timeouts
- widths and heights

## Observability

The harness itself should emit structured logs.

At minimum:

- adapter selected
- launch command
- env overrides
- discovered window ids
- resize operations
- trace bundle timing
- imported artifact paths
- timeout or retry events

Rust logging stack:

- `tracing`
- `tracing-subscriber`
- optional JSON log mode

## Error Model

Errors should be explicit and typed.

Examples:

- target root missing
- required binary missing
- window not found
- trace timeout
- artifact import missing
- process exited early
- report serialization failure

Use `thiserror` for crate-local error types and convert to `anyhow` at the CLI
boundary.

## Migration Plan

### Phase 1: Completed

- create the Cargo workspace
- implement `auto-ui-cli`, `auto-ui-core`, `auto-ui-driver-x11`
- port the current `rust-chatbot` width scan behavior

### Phase 2: Completed

- implement `auto-ui-adapter-gpui`
- support startup-driven env-based runs
- import gpui-generated CSV/log/markdown artifacts

### Phase 3: Remaining

- stabilize the TOML scenario format now that scenario files exist
- reduce or remove hard-coded default session lists from convenience commands
- stabilize `report.json` schema

### Phase 4: Remaining

- add richer summaries and inspection tools
- add alternative window drivers
- consider packaging and crates.io publication

## Test Strategy

### Unit tests

- trace parsing
- config resolution
- artifact schema serialization
- path resolution

### Integration tests

- fake adapter with deterministic outputs
- fake window driver for orchestration logic
- real adapter smoke tests behind ignored test flags

### Golden tests

- expected JSON report snapshots
- expected imported artifact manifests

## Risks

- Linux desktop tooling remains brittle if the app has no startup automation
  hooks.
- Window-title matching can be ambiguous across multiple app instances.
- Keeping a stable schema while supporting both imported and generated artifacts
  will require discipline.
- `gpui-component-testing` scenarios may differ enough that multiple sub-modes
  are needed inside one adapter.

## Open Questions

- Should the initial Rust workspace be a single binary crate first, then split,
  or start as a workspace immediately?
- Should the CLI own scenario discovery, or should adapters expose it entirely?
- Do we want screenshots in startup-driven gpui runs by default, or keep them
  opt-in?
- Should build integration remain out-of-process permanently, or become an
  optional adapter capability later?

## Recommendation

The migration path described above is now in place. The best next work is:

1. Stabilize the scenario and report schemas.
2. Add smoke and golden tests for both adapters.
3. Improve driver coverage and packaging without undoing the current adapter split.
