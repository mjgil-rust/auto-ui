# Scenario Reference

Complete reference for all scenario configuration fields supported by `auto-ui`.

## Top-Level Fields

All scenario files support these top-level fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version` | string | Yes | Scenario format version. Currently `"1"`. |
| `target` | string | Yes | Target adapter: `rust_chatbot` or `gpui_component_testing`. |
| `scenario` | string | Yes | Scenario name within the target. |
| `mode` | string | No | Execution mode: `startup_driven`, `interactive_window`, or `hybrid`. Defaults to target-specific default. |
| `output_dir` | string | No | Override the default output directory path. |

## rust-chatbot Fields

### `rust_chatbot` / `debug` Scenario

**Required config section:**
```toml
[app]
root = "/path/to/rust-chatbot"
provider = "codex"  # or "claude", "gemini"

[window]
widths = [520, 900, 1000]  # At least one width required
height = 900
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `app.root` | string | env/auto-discover | Path to rust-chatbot root. Overrides `AUTO_UI_APP_ROOT` env var. |
| `app.provider` | string | `"codex"` | Provider: `claude`, `codex`, or `gemini`. |
| `app.instance` | integer | None | Provider instance identifier. |
| `app.max_sessions` | integer | `8` | Maximum sessions to list in session picker. |
| `app.session_id` | string | None | Start in a specific session by ID. |
| `app.include_hidden` | boolean | `false` | Include hidden sessions in session list. |
| `window.widths` | array[integer] | `[520, 900, 1000]` | Widths to measure. At least one required. |
| `window.height` | integer | `900` | Window height for measurements. |
| `window.launch` | boolean | `true` | Launch a new window if none exists. |
| `window.keep_front` | boolean | `false` | Do not lower window behind others. |
| `runtime.window_timeout` | float | `15.0` | Seconds to wait for window to appear. |
| `runtime.trace_timeout` | float | `5.0` | Seconds to wait for trace output. |
| `runtime.settle` | float | `0.7` | Seconds to wait for UI to settle after resize. |

### `rust_chatbot` / `header_debug` Scenario

**Additional fields:**
```toml
[app]
session_name = "rcc-header"  # Start in session by name

[capture]
header_height = 140  # Header region height for crop metrics
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `app.session_name` | string | None | Start in session by name (for header debug). |
| `capture.header_height` | integer | `140` | Header region height for crop metrics. |
| `runtime.trace_timeout` | float | `8.0` | Trace timeout (higher than debug). |

### `rust_chatbot` / `prompt_debug` Scenario

**Additional fields:**
```toml
[window]
width = 700  # Single width for prompt debug

[prompt]
text = "Generate markdown..."  # Prompt text to submit
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `window.width` | integer | `700` | Single window width. |
| `prompt.text` | string | required | Prompt text to submit to session. |

## GPUI Fields

### `gpui_component_testing` / `scroll_matrix` Scenario

```toml
[bench]
variants = ["cached_parent_scroll_markdown", "cached_markdown", ...]
run_ms = 12000
warmup_ms = 1500
scroll_delay_ms = 16
scroll_step_px = 40
window_title_prefix = "Auto UI GPUI Scroll Matrix"

[capture]
capture_window = false
settle_ms = 800
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `app.root` | string | env/auto-discover | Path to gpui-component-testing root. |
| `app.example` | string | See below | Example binary name. |
| `bench.variants` | array[string] | Required | Benchmark variants to run. |
| `bench.run_ms` | integer | `12000` | Run duration per variant in milliseconds. |
| `bench.warmup_ms` | integer | `1500` | Warmup duration in milliseconds. |
| `bench.scroll_delay_ms` | integer | `16` | Delay between scroll steps. |
| `bench.scroll_step_px` | integer | `40` | Scroll step size in pixels. |
| `bench.window_title_prefix` | string | Auto UI GPUI Scroll Matrix | Window title prefix for discovery. |
| `capture.capture_window` | boolean | `false` | Capture screenshots during run. |
| `capture.settle_ms` | integer | `800` | Settle time after benchmark. |

**Default example:** `llm_chat_story_style_bench_demo`

### `gpui_component_testing` / `scrollbar_trace` Scenario

```toml
[trace]
run_ms = 4000
warmup_ms = 1000
scroll_delay_ms = 16
scroll_step_px = 40
window_title = "Auto UI GPUI Scrollbar Trace"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `app.example` | string | `llm_chat_story_style_scrollbar_demo` | Example binary name. |
| `trace.run_ms` | integer | `4000` | Trace duration in milliseconds. |
| `trace.warmup_ms` | integer | `1000` | Warmup duration in milliseconds. |
| `trace.scroll_delay_ms` | integer | `16` | Delay between scroll steps. |
| `trace.scroll_step_px` | integer | `40` | Scroll step size in pixels. |
| `trace.window_title` | string | Auto UI GPUI Scrollbar Trace | Window title for discovery. |

### `gpui_component_testing` / `conversation_paint` Scenario

```toml
[bench]
threads = [0, 1, 2, 3]
run_ms = 2500
defer_first_frame = true
window_title_prefix = "Auto UI GPUI Conversation Paint"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `app.example` | string | `llm_chat_conversation_bench_demo` | Example binary name. |
| `bench.threads` | array[integer] | Required | Thread IDs to benchmark. |
| `bench.run_ms` | integer | `2500` | Run duration in milliseconds. |
| `bench.defer_first_frame` | boolean | `true` | Defer first frame rendering. |
| `bench.window_title_prefix` | string | Auto UI GPUI Conversation Paint | Window title prefix. |

## Common Fields

### Headless Mode

For any scenario, add `--headless` to the CLI to run on a private Xvfb display:

```bash
auto-ui run --config scenario.toml --headless --geometry 1280x800x24
```

| CLI Flag | Default | Description |
|----------|---------|-------------|
| `--headless` | false | Run on private Xvfb display with openbox. |
| `--geometry` | `1280x800x24` | Xvfb screen geometry (WxHxD). |

Requires Xvfb and openbox to be installed.

### Output Directory

The output directory contains:
- `report.json` - Run report with events and measurements
- `*.csv` - Benchmark/trace data (GPUI scenarios)
- `*.png` - Window screenshots (if capture enabled)

## Field Validation

`auto-ui validate --config <file.toml>` can catch:
- Missing required fields
- Empty arrays where at least one element is required
- Unknown fields (TOML with `deny_unknown_fields` equivalent)
- Invalid enum values (unknown provider, mode, etc.)

## Example Files

See `examples/` directory for complete working examples:
- `rust-chatbot-debug.toml` - Width measurement debug session
- `rust-chatbot-header-debug.toml` - Header capture session
- `rust-chatbot-prompt-debug.toml` - Prompt timing session
- `gpui-scroll-matrix.toml` - Scroll matrix benchmark
- `gpui-scrollbar-trace.toml` - Scrollbar trace scenario
- `gpui-conversation-paint.toml` - Conversation paint benchmark