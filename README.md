# auto-ui

Standalone UI automation helpers for the `rust-chatbot` desktop app.

This repo extracts the auto-UI debug scripts from the main app repo so they can
be versioned separately while still driving an existing `rust-chatbot`
checkout.

## What it contains

- `scripts/auto_ui_debug.py`: cycles sessions across window widths and collects
  screenshots plus `ui_auto_debug` trace bundles
- `scripts/auto_ui_header_debug.py`: launches a single session and captures
  header-focused crops and metrics
- `scripts/auto_ui_common.py`: shared window, trace, and filesystem helpers

## Requirements

- Linux desktop session with `DISPLAY` set
- `xdotool`
- `wmctrl`
- ImageMagick tools: `import`, `convert`, `identify`
- A separate `rust-chatbot` checkout with release binaries already built

The extracted scripts do not build `rust-chatbot` themselves. Build the target
app through your lightweight build path first, then point these scripts at that
checkout.

## Target app path

The scripts resolve the target app root in this order:

1. `--app-root /path/to/rust-chatbot`
2. `RUST_CHATBOT_APP_ROOT`
3. `RUST_CHATBOT_ROOT`
4. sibling checkout at `/home/m/git/rust-chatbot` relative to this repo

## Examples

```bash
python3 scripts/auto_ui_debug.py --provider codex
```

```bash
python3 scripts/auto_ui_debug.py \
  --app-root /home/m/git/rust-chatbot \
  --provider codex \
  --session-id 64ee1661-b53f-4134-8855-cc2c25a06ddd
```

```bash
python3 scripts/auto_ui_header_debug.py \
  --app-root /home/m/git/rust-chatbot \
  --provider codex \
  --session-name "rcc-header"
```

Artifacts land under `tmp/` in this repo unless `--output-dir` is provided.

## Desktop Behavior

The automation should strive to be minimally conflicting with a desktop user's
active workflow.

- Prefer startup-driven automation over mouse-driven interaction when the app
  supports it.
- Do not move the mouse pointer unless a scenario explicitly opts in and there
  is no reliable alternative.
- Do not leave automation windows in the foreground by default. Launch them,
  attach as needed, then lower them again.
- Treat `keep_front = true` style behavior as an explicit opt-in.
- Send keys only to the verified target window, not to the user's currently
  focused app.
- Avoid switching workspaces, sending global shortcuts, or using the clipboard
  when another mechanism is available.
