# rust-chatbot `header-debug`

These examples capture header-focused artifacts for one `rust-chatbot` session.

## Capture by session name

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui header-debug \
  --app-root /home/m/git/rust-chatbot \
  --provider codex \
  --session-name "rcc-header"
```

## Generic runner with scenario file

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui run --config examples/rust-chatbot-header-debug.toml
```

What it does:

- launches directly into the target session
- captures full-window screenshots
- writes top-strip and focused header crops
- writes artifacts under `tmp/auto-ui-header-debug-*`

## Capture by session id

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui header-debug \
  --app-root /home/m/git/rust-chatbot \
  --provider codex \
  --session-id 64ee1661-b53f-4134-8855-cc2c25a06ddd
```

## Override widths and crop height

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui header-debug \
  --app-root /home/m/git/rust-chatbot \
  --provider codex \
  --session-name "rcc-header" \
  --widths 520,900,1000 \
  --header-height 140 \
  --output-dir /home/m/git/auto-ui/tmp/header-manual
```
