# `gpui-component-testing` Planned Usage

This file shows the intended usage once the `gpui-component-testing` adapter is
implemented. It is not supported by the current CLI yet.

Target repo:

- `/home/m/git/gpui-component-testing`

## Planned example: scroll matrix

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui run \
  --target gpui-component-testing \
  --scenario scroll-matrix \
  --app-root /home/m/git/gpui-component-testing
```

Expected adapter behavior:

- resolve a prebuilt example binary or configured launch command
- inject env vars such as `BENCH_AUTO_SCROLL`, `BENCH_SCROLL_WARMUP_MS`,
  `BENCH_SCROLL_TICK_MS`, and `BENCH_SCROLL_STEP_PX`
- let the app run autonomously
- import generated CSV/log/markdown artifacts into the run report

## Planned example: scrollbar trace

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui run \
  --target gpui-component-testing \
  --scenario scrollbar-trace \
  --app-root /home/m/git/gpui-component-testing
```

Expected adapter behavior:

- set `GPUI_COMPONENT_SCROLLBAR_TRACE=1`
- set the `SCROLLBAR_DEMO_*` env vars for duration, warmup, tick, and step
- wait for process completion
- import the trace logs and generated summary
