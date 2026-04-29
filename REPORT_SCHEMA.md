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

## Artifact Rows

Each `artifacts[]` entry has:

- `kind`: stable artifact category such as `progress_log`, `summary_markdown`, or `window_screenshot`.
- `path`: on-disk path written by the run. Always absolute and normalized by `normalize_artifact_path()`.
- `description`: optional human-readable label.
- `metadata`: optional object for scenario-specific qualifiers such as variant or width.

## Path Semantics

Artifact paths in `artifacts[].path` are always absolute paths. Relative paths provided to `add_artifact()` are automatically resolved from the current working directory and canonicalized. This ensures reports contain portable, unambiguous path references.

## Compatibility Rules

- New top-level fields require a schema version bump.
- Adapter-specific expansion belongs under `details`, `measurements[]`, `events[]`, or artifact `metadata` unless it is clearly shared across targets.
- Consumers should treat unknown nested keys as additive.
