# rust-chatbot `prompt_debug`

This scenario launches a targeted `rust-chatbot` session in background mode,
submits a prompt through `chatbot-ctl send`, and waits for the response timing
events in the tracing log.

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui run --config examples/rust-chatbot-prompt-debug.toml
```

What it does:

- launches a fresh `rust-chatbot` window for one session
- keeps the automation window in the background unless `keep_front = true`
- sends the prompt without typing into the window
- waits for `ai_response_end` and `assistant_markdown_upgrade_latency*`
- writes artifacts under `tmp/auto-ui-prompt-debug-*`
