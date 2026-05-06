# Scenario Reference

This file documents the current scenario TOML shape.

## `rust_chatbot` / `debug`

**Required config section:**
```toml
[app]
root = "/path/to/rust-chatbot"
provider = "codex"  # or "claude", "gemini", "geminiforge", "minimaxforge"

[window]
widths = [520, 900, 1000]  # At least one width required
height = 900
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `app.root` | string | env/auto-discover | Path to rust-chatbot root. Overrides `AUTO_UI_APP_ROOT` env var. |
| `app.provider` | string | `"codex"` | Provider: `claude`, `codex`, `gemini`, `geminiforge`, or `minimaxforge`. |
| `app.instance` | integer | None | Provider instance identifier. |
| `app.max_sessions` | integer | `8` | Maximum sessions to list in session picker. |
| `app.session_id` | string | None | Start in a specific session by ID. |
| `app.include_hidden` | boolean | `false` | Include hidden sessions in session list. |
| `window.widths` | array[integer] | `[520, 900, 1000]` | Widths to measure. At least one required. |
| `window.height` | integer | `900` | Target window height. |
| `window.keep_front` | boolean | `false` | Keep the automation window in front instead of lowering it. |
| `runtime.launch` | boolean | `true` | Launch a fresh window instead of only reusing an existing one. |
| `runtime.window_timeout` | float | `15.0` | Seconds to wait for a window to appear. |
| `runtime.trace_timeout` | float | `5.0` | Seconds to wait for trace capture after a resize cycle. |
| `runtime.settle` | float | `0.7` | Seconds to wait after resize/focus actions. |
| `output_dir` | string | auto-generated | Optional output directory override. |

## `rust_chatbot` / `header_debug`

```toml
target = "rust_chatbot"
scenario = "header_debug"

[app]
root = "/path/to/rust-chatbot"
provider = "codex"
session_id = "64ee1661-b53f-4134-8855-cc2c25a06ddd"

[window]
widths = [520, 900, 1000]
height = 900
header_height = 140
```

The field meanings match `debug`, with these additions:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `app.session_name` | string | None | Resolve the session by name instead of ID. |
| `window.header_height` | integer | `140` | Pixel height of the top crop. |

## `rust_chatbot` / `prompt_debug`

```toml
target = "rust_chatbot"
scenario = "prompt_debug"

[app]
root = "/path/to/rust-chatbot"
provider = "codex"
session_id = "64ee1661-b53f-4134-8855-cc2c25a06ddd"

[window]
width = 700
height = 900

[prompt]
text = "Generate about 6 KB of markdown."
```

Extra prompt-debug fields:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `window.width` | integer | `700` | Target window width for the run. |
| `prompt.text` | string | empty | Prompt to send into the target session. |
| `runtime.trace_timeout` | float | `45.0` | Seconds to wait for the response trace bundle. |

## `gpui_component_testing`

GPUI scenarios use the same top-level `target` and `scenario` fields but a
different adapter-owned config surface. See the shipped example TOML files for
the exact current shape.

## Validation notes

Validation can fail for:

- Missing required fields
- Invalid enum values (unknown provider, mode, etc.)
- Empty width lists for `debug`
- Unsupported scenario names
