# Design Document: macOS Support for auto-ui

## Status

Implemented. All phases (1–5) are complete.

- Phase 1 (Trait Extraction & X11 Refactor): ✅ Complete
- Phase 2 (ImageMagick Elimination): ✅ Complete
- Phase 3 (Adapter Decoupling): ✅ Complete
- Phase 4 (macOS Driver): ✅ Complete — `auto-ui-driver-macos` compiles and passes unit tests on macOS
- Phase 5 (Validation, CI, Documentation): ✅ Complete — `macos-latest` CI job added, documentation updated, `cargo clippy`/`fmt` pass cleanly

Remaining validation gates (Linux live smoke tests, macOS end-to-end live tests with target apps) are tracked in `docs/tasks.md` and require platform-specific hardware/target binaries.

## Context

`auto-ui` is a Rust workspace for desktop UI automation. It currently supports Linux/X11 only. The architecture has the *shape* of a cross-platform tool—generic adapter traits, a stable report schema, and pluggable execution modes—but the production code paths are hard-wired to X11 via `xdotool`, `wmctrl`, ImageMagick, and `Xvfb`.

This document describes the changes required to make `auto-ui` run natively on macOS while keeping the Linux/X11 path intact and improving the overall portability of the codebase.

## Goals

1. Add a first-class macOS window automation driver.
2. Keep the existing Linux/X11 driver fully functional.
3. Make target adapters (`rust-chatbot`, `gpui`) platform-agnostic.
4. Remove the external ImageMagick dependency by using Rust-native image processing.
5. Support compilation and unit-test execution on macOS CI.
6. Preserve the existing Desktop Coexistence Policy on both platforms.

## Non-Goals

1. **Wayland support** — out of scope for this document.
2. **Windows support** — we will not add `cfg(windows)` blocks unless they are free consequences of the trait refactor.
3. **Replacing app-specific adapter logic** — `rust-chatbot` session parsing and `gpui` CSV import remain adapter-owned.
4. **Auto-compiling target apps** — the harness still expects pre-built binaries.
5. **Headless virtual display on macOS** — macOS does not have an `Xvfb` equivalent. `--headless` will be unsupported on macOS for v1.

## Current State Analysis

### Platform-Specific Surface Area

The following table maps every X11/Linux-specific dependency to its current location and macOS replacement strategy.

| Capability | Current Implementation | Crate | macOS Replacement |
|---|---|---|---|
| Window discovery by PID/title | `xdotool search`, `wmctrl -l` | `auto-ui-driver-x11` | CoreGraphics `CGWindowListCopyWindowInfo` |
| Window geometry query | `xdotool getwindowgeometry` | `auto-ui-driver-x11` | Accessibility `AXUIElement` + `CGWindowList` |
| Resize / move | `xdotool windowsize`, `windowmove` | `auto-ui-driver-x11` | Accessibility `kAXSizeAttribute`, `kAXPositionAttribute` |
| Activate window | `xdotool windowactivate` | `auto-ui-driver-x11` | `NSRunningApplication.activate()` or Accessibility |
| Lower window | `wmctrl -b add,below` | `auto-ui-driver-x11` | Accessibility `kAXMainAttribute = false` + AppKit ordering |
| Screenshot | ImageMagick `import -window` | `auto-ui-driver-x11` | `CGWindowListCreateImage` or `screencapture` |
| Key input | `xdotool key`, `type` | `auto-ui-driver-x11` | `CGEventPost` (CoreGraphics) |
| Image metrics (stddev, colors) | ImageMagick `convert`, `identify` | `auto-ui-driver-x11` | Rust `image` crate |
| Headless display | `Xvfb` + `openbox` | `auto-ui-core` | **Unsupported** in v1 |
| Display session check | `DISPLAY` env var | `auto-ui-core` | Skip on macOS / check Aqua session |
| Process group cleanup | `libc::kill(-pgid, SIGTERM)` | `auto-ui-core` | macOS `killpg` or `signal::kill` |

### Architectural Gaps

1. **Unused abstractions.** The `WindowDriver` trait exists in `auto-ui-driver-x11` but is only implemented by `FakeWindowDriver`. Production adapters call free functions (`x11::find_window_id`, `x11::capture_window_screenshot`) directly.
2. **Unused orchestration.** `InteractiveWindowOrchestrator`, `StartupOrchestrator`, and `HybridOrchestrator` in `auto-ui-core` are well-designed stubs. Adapters reimplement window lifecycle logic inline.
3. **Unused `TargetAdapter` trait.** The CLI hardcodes target dispatch in `run_cmd.rs` rather than using `AdapterRegistry`.
4. **Tight coupling.** Both adapter crates declare a direct Cargo dependency on `auto-ui-driver-x11`.

## Proposed Architecture

### High-Level Crate Graph

```text
auto-ui/
├── Cargo.toml
├── crates/
│   ├── auto-ui-cli/
│   ├── auto-ui-core/          <-- WindowDriver trait moves here
│   ├── auto-ui-artifacts/
│   ├── auto-ui-driver-x11/    <-- implements WindowDriver
│   ├── auto-ui-driver-macos/  <-- NEW: implements WindowDriver
│   ├── auto-ui-adapter-rust-chatbot/  <-- no longer depends on driver-x11
│   └── auto-ui-adapter-gpui/          <-- no longer depends on driver-x11
```

### Trait Refactor: `WindowDriver`

The trait must be moved to `auto-ui-core` and expanded to cover all current X11 free-function usage.

**New location:** `auto-ui-core/src/window_driver.rs`

```rust
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::time::Duration;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VisualMetric {
    pub stddev: f64,
    pub colors: f64,
}

/// Trait for pluggable window automation backends.
pub trait WindowDriver: Send + Sync {
    /// Validate that required OS-level dependencies are present.
    fn check_required_tools(&self) -> Result<()>;

    // -- Discovery --
    fn find_windows(&self, title_substring: &str) -> Result<Vec<String>>;
    fn find_windows_for_pid(&self, pid: i32) -> Result<Vec<String>>;
    fn find_window(&self, title_substring: &str, timeout: Duration) -> Result<Option<String>>;
    fn find_window_for_pid(&self, pid: i32, timeout: Duration) -> Result<Option<String>>;
    fn wait_for_new_window(
        &self,
        title_substring: &str,
        before_ids: &HashSet<String>,
        timeout: Duration,
    ) -> Result<Option<String>>;
    fn get_active_window(&self) -> Result<Option<String>>;

    // -- Geometry & State --
    fn get_geometry(&self, window_id: &str) -> Result<WindowGeometry>;
    fn resize(&self, window_id: &str, width: u32, height: u32) -> Result<WindowGeometry>;
    fn set_geometry(&self, window_id: &str, geometry: &WindowGeometry) -> Result<WindowGeometry>;
    fn activate(&self, window_id: &str) -> Result<()>;
    fn lower(&self, window_id: &str) -> Result<()>;
    fn background(&self, window_id: &str, restore_window_id: Option<&str>) -> Result<()>;
    fn window_exists(&self, window_id: &str) -> Result<bool>;

    // -- Input --
    fn press_key(&self, window_id: &str, key: &str) -> Result<()>;
    fn type_text(&self, window_id: &str, text: &str) -> Result<()>;
    fn clear_search(&self, window_id: &str) -> Result<()>;
    fn select_session(&self, window_id: &str, session_name: &str) -> Result<()>;

    // -- Capture --
    fn screenshot(&self, window_id: &str, output_path: &Path) -> Result<()>;

    // -- Image Processing (previously ImageMagick) --
    fn crop_metric(&self, image_path: &Path, x: i32, y: i32, width: i32, height: i32) -> Result<VisualMetric>;
    fn image_size(&self, image_path: &Path) -> Result<(i32, i32)>;
}

/// Default heuristic used by both platforms.
pub fn heuristic_text_visible(metric: &VisualMetric) -> bool {
    metric.stddev >= 0.01 || metric.colors >= 16.0
}
```

Notes:
- `auto-ui-driver-x11` will provide `X11WindowDriver` implementing this trait.
- `auto-ui-driver-macos` will provide `MacOsWindowDriver` implementing this trait.
- `heuristic_text_visible` moves to `auto-ui-core` so adapters do not depend on a specific driver for business logic.

### Image Processing Migration

Replace ImageMagick with the Rust `image` crate.

**Crate:** `auto-ui-core` (or a new `auto-ui-image` crate if the dependency graph demands it).

**Operations to reimplement:**

1. **`image_size(path)`**
   ```rust
   use image::GenericImageView;
   let img = image::open(path)?;
   Ok((img.width() as i32, img.height() as i32))
   ```

2. **`crop_metric(path, x, y, w, h)`**
   - Open image, crop to sub-region.
   - Compute grayscale standard deviation by iterating pixels.
   - Count unique colors with a `HashSet<(u8,u8,u8)>` or quantization.
   - Return `VisualMetric { stddev, colors }`.

3. **`screenshot(window_id, path)`**
   - **X11:** Keep using `import` CLI *for now*, or migrate to `xcb`/`x11rb` later. The trait allows per-platform impl.
   - **macOS:** Use `CGWindowListCreateImage` to capture by window ID, then save via `image::save_buffer`.

**Dependency addition:**
```toml
[dependencies]
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
```

### Adapter Refactor

Both adapters must stop depending on `auto-ui-driver-x11` and accept a `&dyn WindowDriver`.

**1. Remove driver dependency from adapters**

```toml
# auto-ui-adapter-rust-chatbot/Cargo.toml
[dependencies]
anyhow.workspace = true
auto-ui-artifacts = { path = "../auto-ui-artifacts" }
auto-ui-core = { path = "../auto-ui-core" }
# REMOVED: auto-ui-driver-x11
```

**2. Update adapter entry points**

Current:
```rust
pub fn run_debug(config: DebugConfig) -> Result<CompletedRun>
```

Target:
```rust
pub fn run_debug(
    config: DebugConfig,
    driver: &dyn WindowDriver,
) -> Result<CompletedRun>
```

Same for `run_header_debug`, `run_prompt_debug`, `run_scroll_matrix`, `run_scrollbar_trace`, `run_conversation_paint`.

**3. Mechanical replacements**

| Old Call | New Call |
|---|---|
| `x11::find_window_id(...)` | `driver.find_window(...)` |
| `x11::get_window_geometry(...)` | `driver.get_geometry(...)` |
| `x11::resize_window(...)` | `driver.resize(...)` |
| `x11::activate_window(...)` | `driver.activate(...)` |
| `x11::background_window(...)` | `driver.background(...)` |
| `x11::capture_window_screenshot(...)` | `driver.screenshot(...)` |
| `x11::crop_metric(...)` | `driver.crop_metric(...)` |
| `x11::image_size(...)` | `driver.image_size(...)` |
| `x11::press_key(...)` | `driver.press_key(...)` |
| `x11::type_text(...)` | `driver.type_text(...)` |

### CLI Wiring

`auto-ui-cli` becomes the place where the concrete driver is instantiated and injected.

```rust
// auto-ui-cli/src/run_cmd.rs
use auto_ui_core::WindowDriver;

#[cfg(target_os = "linux")]
fn build_driver() -> Result<Box<dyn WindowDriver>> {
    let driver = auto_ui_driver_x11::X11WindowDriver::new();
    driver.check_required_tools()?;
    Ok(Box::new(driver))
}

#[cfg(target_os = "macos")]
fn build_driver() -> Result<Box<dyn WindowDriver>> {
    let driver = auto_ui_driver_macos::MacOsWindowDriver::new();
    driver.check_required_tools()?;
    Ok(Box::new(driver))
}

pub fn run(args: Args) -> Result<()> {
    let driver = build_driver()?;
    // ... pass &*driver into adapter functions
}
```

**Cargo.toml conditional dependencies:**
```toml
[target.'cfg(target_os = "linux")'.dependencies]
auto-ui-driver-x11 = { path = "../auto-ui-driver-x11" }

[target.'cfg(target_os = "macos")'.dependencies]
auto-ui-driver-macos = { path = "../auto-ui-driver-macos" }
```

### macOS Driver Implementation (`auto-ui-driver-macos`)

This crate will use a mix of CoreGraphics and Accessibility APIs.

#### Window Discovery

```rust
use core_foundation::array::CFArray;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::window::{CGWindowListCopyWindowInfo, kCGWindowListOptionAll};

fn find_windows(title_substring: &str) -> Result<Vec<String>> {
    // CGWindowListCopyWindowInfo returns CFDictionary per window.
    // Filter by kCGWindowName or kCGWindowOwnerPID.
    // Return window numbers (kCGWindowNumber) as String IDs.
}
```

#### Geometry

```rust
use accessibility::{AXUIElement, AXAttribute};

fn get_geometry(window_id: &str) -> Result<WindowGeometry> {
    // 1. Convert window_id (number) to AXUIElement via system-wide element.
    // 2. Read kAXPositionAttribute and kAXSizeAttribute.
}
```

#### Screenshot

```rust
use core_graphics::window::CGWindowListCreateImage;
use core_graphics::geometry::CGRect;

fn screenshot(window_id: &str, path: &Path) -> Result<()> {
    let window_id_num: u32 = window_id.parse()?;
    let cg_image = CGWindowListCreateImage(
        CGRect::null(),
        kCGWindowListOptionIncludingWindow,
        window_id_num as usize,
        kCGWindowImageBoundsIgnoreFraming,
    );
    // Convert CGImage to PNG bytes and write.
}
```

#### Key Input

```rust
use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};
use core_graphics::event_source::CGEventSource;

fn press_key(window_id: &str, key: &str) -> Result<()> {
    // Map key string to CGKeyCode.
    // Create CGEventSource::newHIDSystemState().
    // Post keyboard down + up via CGEventPost.
    // NOTE: Requires Accessibility permissions in System Preferences.
}
```

#### Permissions

macOS requires the running process to have **Accessibility** permissions to:
- Query/change window geometry of other apps.
- Send synthetic input events.

The `check_required_tools()` implementation on macOS should verify accessibility access and return a helpful error if denied:

```rust
fn check_required_tools(&self) -> Result<()> {
    if !accessibility::is_enabled() {
        bail!(
            "Accessibility access is required for window automation on macOS. \
             Enable it in System Settings > Privacy & Security > Accessibility."
        );
    }
    Ok(())
}
```

### Headless Display Policy

`HeadlessDisplay` in `auto-ui-core` is Linux-only. The proposed behavior:

| Platform | `--headless` | Behavior |
|---|---|---|
| Linux | Not provided | Run on user's current `$DISPLAY`. |
| Linux | Provided | Spawn `Xvfb` + `openbox` as today. |
| macOS | Not provided | Run on user's current Aqua session. |
| macOS | Provided | **Error:** "Headless mode is not supported on macOS." |

The `run_cmd.rs` `Args` struct keeps the `--headless` flag, but the `run()` function gates it:

```rust
if args.headless {
    #[cfg(target_os = "linux")]
    {
        let _hd = HeadlessDisplay::start(&args.geometry)?;
    }
    #[cfg(target_os = "macos")]
    {
        bail!("--headless is not supported on macOS");
    }
}
```

### `ensure_display` Update

Replace the X11-centric `DISPLAY` check with a platform-aware helper.

```rust
pub fn ensure_display(app_name: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("DISPLAY").is_none() {
            bail!("DISPLAY is not set. Run this from a desktop terminal in the same X session as {app_name}.");
        }
    }
    #[cfg(target_os = "macos")]
    {
        // Aqua session is implied by running in a graphical context.
        // Optionally verify we are not in a pure SSH session.
        if std::env::var_os("SSH_CONNECTION").is_some() {
            bail!("Running over SSH without Aqua session. Run from a local macOS terminal.");
        }
    }
    Ok(())
}
```

## Implementation Plan

### Phase 1: Trait Extraction & X11 Refactor (no macOS code yet)

1. Move `WindowDriver` trait and `WindowGeometry` to `auto-ui-core`.
2. Move `heuristic_text_visible` to `auto-ui-core`.
3. Create `X11WindowDriver` in `auto-ui-driver-x11` that implements `WindowDriver` by wrapping the existing free functions.
4. Update `auto-ui-driver-x11` tests to test through `X11WindowDriver`.
5. **Verify:** `cargo test --workspace` passes on Linux.

### Phase 2: ImageMagick → Rust `image` Crate

1. Add `image` to `auto-ui-core` dependencies.
2. Reimplement `crop_metric`, `image_size` in pure Rust.
3. Update `X11WindowDriver` to use the Rust impls for image processing.
4. Remove `convert` and `identify` from `REQUIRED_X11_TOOLS`.
5. **Verify:** Screenshot metrics still work on Linux live tests.

### Phase 3: Adapter Decoupling

1. Remove `auto-ui-driver-x11` from adapter `Cargo.toml` files.
2. Update all adapter public functions to accept `&dyn WindowDriver`.
3. Replace all `x11::` calls with `driver.` method calls.
4. Update `auto-ui-cli` to construct `X11WindowDriver` and inject it.
5. **Verify:** `cargo test --workspace` and live smoke tests pass on Linux.

### Phase 4: macOS Driver

1. Create `crates/auto-ui-driver-macos/`.
2. Implement `MacOsWindowDriver`:
   - `find_windows` / `find_windows_for_pid` via CoreGraphics.
   - `get_geometry` / `resize` / `set_geometry` via Accessibility.
   - `activate` / `lower` via Accessibility + AppKit.
   - `screenshot` via `CGWindowListCreateImage`.
   - `press_key` / `type_text` via `CGEventPost`.
   - `crop_metric` / `image_size` via the shared Rust `image` impl.
3. Add macOS conditional compilation to `auto-ui-cli`.
4. Update `ensure_display` for macOS.

### Phase 5: Validation & CI

1. Add `macos-latest` job to `.github/workflows/ci.yml`.
2. Ensure `cargo check`, `cargo clippy`, and `cargo test` pass on macOS.
3. Update `README.md` with macOS requirements (Accessibility permissions, no `--headless`).
4. Update `DESIGN.md` to reflect that cross-platform parity is now a goal.

## Risks & Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| macOS Accessibility permission UX | High | `check_required_tools()` returns a clear, actionable error linking to System Settings. |
| Sandboxed / signed app restrictions | Medium | Document that `auto-ui` must not be sandboxed. Use `cargo run` or unsigned binaries for local automation. |
| CoreGraphics window IDs are transient | Medium | Window IDs on macOS (CGWindowNumber) can change. Prefer PID + title matching with retry logic already in `retry.rs`. |
| `CGEventPost` key mapping complexity | Medium | Start with a small keymap (letters, numbers, Return, Escape, ctrl+*, cmd+*). Expand as needed. |
| Image crate performance on large screenshots | Low | Test with 4K captures. If slow, optimize or use platform-specific fast paths. |
| Adapter refactor breaks Linux | Medium | Keep Linux CI green at every phase. Run live smoke tests on Linux before declaring Phase 3 complete. |

## Open Questions

1. Should we promote `InteractiveWindowOrchestrator` to the primary code path *before* adding macOS, or after?
   - **Recommendation:** After. The orchestrator refactor is valuable but independent. Doing it first would delay macOS without benefit.
2. Should `TargetAdapter` trait adoption happen as part of this work?
   - **Recommendation:** No. It is a larger architectural cleanup. Keep the CLI hardcoded dispatch for now; the `WindowDriver` refactor is the critical portability gate.
3. Do we need to support Apple Silicon vs Intel differently?
   - **Recommendation:** No. CoreGraphics and Accessibility APIs are identical on both architectures. Cargo handles architecture natively.

## Appendix: macOS API Reference

### CoreGraphics Window Info
```objc
CFArrayRef CGWindowListCopyWindowInfo(CGWindowListOption option, CGWindowID relativeToWindow);
```
Keys of interest:
- `kCGWindowNumber` → window ID
- `kCGWindowOwnerPID` → owner PID
- `kCGWindowName` → title
- `kCGWindowBounds` → `CGRect` with origin/size

### CoreGraphics Screenshot
```objc
CGImageRef CGWindowListCreateImage(
    CGRect screenBounds,
    CGWindowListOption listOption,
    CGWindowID windowID,
    CGWindowImageOption imageOption
);
```

### Accessibility Attributes
- `kAXPositionAttribute`
- `kAXSizeAttribute`
- `kAXMainAttribute`
- `kAXFocusedAttribute`

### Synthetic Events
```objc
CGEventRef CGEventCreateKeyboardEvent(CGEventSourceRef source, CGKeyCode virtualKey, bool keyDown);
void CGEventPost(CGEventTapLocation tap, CGEventRef event);
```

---

*Document version: 1.0*
