# auto-ui

Rust desktop UI automation helpers for the `rust-chatbot` desktop app.

This repo contains the Rust rewrite of the extracted auto-UI tooling from the
main app repo. It drives an existing `rust-chatbot` checkout and keeps build
orchestration outside the automation loop.

## What it contains

- `Cargo.toml`: workspace root
- `crates/auto-ui-cli`: Rust CLI implementation
- `auto-ui debug`: cycles sessions across window widths and collects
  screenshots plus `ui_auto_debug` trace bundles
- `auto-ui header-debug`: launches a single session and captures
  header-focused crops and metrics

## Requirements

- Linux desktop session with `DISPLAY` set
- Rust toolchain installed locally
- `xdotool`
- `wmctrl`
- ImageMagick tools: `import`, `convert`, `identify`
- A separate `rust-chatbot` checkout with release binaries already built

This repo does not build `rust-chatbot` itself. Build the target app through
your lightweight build path first, then point `auto-ui` at that checkout. Build
this repo through the same lightweight path before running the binary.

## Target app path

The CLI resolves the target app root in this order:

1. `--app-root /path/to/rust-chatbot`
2. `RUST_CHATBOT_APP_ROOT`
3. `RUST_CHATBOT_ROOT`
4. sibling checkout at `/home/m/git/rust-chatbot` relative to this repo

## Commands

- `debug`: width scan across sessions
- `header-debug`: single-session header capture flow

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

## Status

The current Rust implementation is focused on parity with the original
`rust-chatbot` flows. The broader generic-adapter split described in
`DESIGN.md` is still planned work.
