# `gpui-component-testing`

The gpui adapter is startup-driven. It launches a prebuilt example binary,
injects the scenario env vars, waits for completion, and writes an `auto-ui`
report plus imported artifacts.

Target repo:

- `/home/m/git/gpui-component-testing`

## Scroll matrix

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui run --config examples/gpui-scroll-matrix.toml
```

What it does:

- launches the configured bench example
- runs each configured variant with `BENCH_*` env vars
- writes CSV, stdout, stderr, ranked TSV/markdown summaries, and report artifacts under `tmp/`

## Scrollbar trace

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui run --config examples/gpui-scrollbar-trace.toml
```

What it does:

- launches the configured scrollbar example
- sets `GPUI_COMPONENT_SCROLLBAR_TRACE=1` and `SCROLLBAR_DEMO_*` env vars
- writes stdout, stderr, summary, and report artifacts under `tmp/`

## Conversation paint

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui run --config examples/gpui-conversation-paint.toml
```

What it does:

- launches the conversation bench example
- runs each configured thread with `BENCH_THREAD`, `BENCH_DURATION_MS`, and `BENCH_DEFER_FIRST_FRAME`
- writes CSV, stdout, stderr, ranked TSV/markdown summaries, and report artifacts under `tmp/`
