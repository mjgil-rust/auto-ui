# rust-chatbot `debug`

These examples use the current Rust CLI and the existing `rust-chatbot`
adapter.

## Basic width scan

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui debug --provider codex
```

## Generic runner with scenario file

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui run --config examples/rust-chatbot-debug.toml
```

What it does:

- resolves the target app root
- launches or reuses a `rust-chatbot` window
- scans the default width set `520,900,1000`
- writes artifacts under `tmp/auto-ui-debug-*`

## Target a specific checkout

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui debug \
  --app-root /home/m/git/rust-chatbot \
  --provider codex
```

## Scan one specific session

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui debug \
  --app-root /home/m/git/rust-chatbot \
  --provider codex \
  --session-id 64ee1661-b53f-4134-8855-cc2c25a06ddd
```

## Reuse an already-open window

Use this if you already launched `rust-chatbot` yourself with
`RUST_CHATBOT_AUTO_UI_DEBUG=1`.

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui debug \
  --app-root /home/m/git/rust-chatbot \
  --provider codex \
  --no-launch
```

## Override widths and output directory

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui debug \
  --app-root /home/m/git/rust-chatbot \
  --provider codex \
  --widths 520,700,900,1200 \
  --height 900 \
  --output-dir /home/m/git/auto-ui/tmp/debug-manual
```
