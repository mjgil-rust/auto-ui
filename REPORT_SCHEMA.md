# Report Schema

`auto-ui` writes a `report.json` file in every output directory.

The top-level schema is stable and versioned. The current version is `1`.

## Export

Print the machine-readable JSON schema:

```bash
cd /home/m/git/auto-ui
target/debug/auto-ui report-schema > report.schema.json
```

## Stable Top-Level Fields

- `schema_version`: stable top-level schema version.
- `tool_version`: `auto-ui` binary version that wrote the report.
- `target`: normalized target id such as `rust_chatbot` or `gpui_component_testing`.
- `scenario`: normalized scenario id.
- `mode`: adapter-selected execution mode.
- `app_root`: resolved target root used for the run.
- `run_id`: run identifier for that output directory.
- `started_at`: RFC 3339 start timestamp.
- `finished_at`: RFC 3339 finish timestamp or `null` while running.
- `status`: `running`, `ok`, or `error`.
- `status_message`: error text when present.
- `artifacts`: structured artifact references with stable `kind` and `path` fields.
- `measurements`: shared measurement rows. Adapters may add keys inside each row.
- `events`: timeline rows. Adapters may add keys inside each row.
- `details`: adapter-specific structured JSON that stays nested instead of promoting every target-specific field to the top level.

## Event Rows

Each `events[]` entry has a `kind` field identifying the event type and a `timestamp` in RFC 3339 format. Adapters may add extra keys per event type.

### Lifecycle Events (`kind: "lifecycle"`)

Emitted for major lifecycle transitions:

- `phase`: one of `prepare`, `launch`, `collect`, `stop`
- `message`: human-readable description

### Retry Events (`kind: "retry"`)

Emitted when an operation is retried:

- `operation`: name of the operation being retried (e.g., `window_discovery`)
- `attempt`: current attempt number (1-indexed)
- `max_attempts`: maximum attempts before giving up

### Timeout Events (`kind: "timeout"`)

Emitted when an operation times out:

- `operation`: name of the operation that timed out
- `timeout_secs`: configured timeout in seconds

### Foreground Control Events (`kind: "foreground_control"`)

Emitted for window activation and lowering:

- `action`: one of `activate`, `lower`
- `window_id`: X11 window ID

### Import Events (`kind: "import"`)

Emitted when artifacts are imported into the report:

- `artifact_kind`: stable artifact category
- `path`: on-disk path of the imported file

### Window Events (`kind: "window"`)

Emitted for window operations:

- `action`: operation performed (e.g., `resize`, `screenshot`, `focus`)
- `window_id`: X11 window ID
- `details`: additional operation-specific data

## Artifact Rows

Each `artifacts[]` entry has:

- `kind`: stable artifact category such as `progress_log`, `summary_markdown`, or `window_screenshot`.
- `path`: on-disk path written by the run. Always absolute and normalized by `normalize_artifact_path()`.
- `source`: artifact origin, either `"generated"` (created by harness) or `"imported"` (from target app). Defaults to `"generated"` if omitted.
- `description`: optional human-readable label.
- `metadata`: optional object for scenario-specific qualifiers such as variant or width.

### Source Field

The `source` field distinguishes artifacts by origin:

- **`generated`**: Artifacts created by the auto-ui harness (e.g., progress logs, harness-triggered screenshots).
- **`imported`**: Artifacts imported from the target app (e.g., target's trace log, CSV output files).

This distinction helps consumers understand artifact provenance and whether to expect the harness or the target app as the author.

## Path Semantics

Artifact paths in `artifacts[].path` are always absolute paths. Relative paths provided to `add_artifact()` are automatically resolved from the current working directory and canonicalized. This ensures reports contain portable, unambiguous path references.

## Compatibility Rules

- New top-level fields require a schema version bump.
- The `source` field is optional and defaults to `"generated"` for backward compatibility with existing reports.
- Adapter-specific expansion belongs under `details`, `measurements[]`, `events[]`, or artifact `metadata` unless it is clearly shared across targets.
- Consumers should treat unknown nested keys as additive.
