# Remaining Work

This document covers the post-migration backlog for `auto-ui`.

The architectural migration is complete. The items below are the remaining
hardening, polish, and expansion work.

## 1. Stabilization

- Add workspace-level smoke tests for:
  - `rust-chatbot` `debug`
  - `rust-chatbot` `header_debug`
  - `gpui-component-testing` `scroll_matrix`
  - `gpui-component-testing` `scrollbar_trace`
- Add golden tests for `report.json` shape so schema drift is caught early.
- Add validation for scenario TOML files before adapter execution.
- ~~Tighten error messages around missing binaries, missing env hooks, and missing
  target roots.~~ (GPUI adapter now bails on missing parent instead of silently
  falling back to `/`; CSV parse failures are logged with row counts.)

## 2. Report Schema Hardening

- Decide how much adapter-specific detail should remain nested under `details`
  versus promoted to shared top-level fields.

## 3. GPUI Coverage

- Expand optional screenshot capture for gpui runs where visual inspection
  matters.
- Decide whether the adapter should support custom commands in addition to
  example binaries.

## 4. Driver Expansion

- Add a non-X11 path for environments where `xdotool` and `wmctrl` are not
  available.
- Evaluate a Wayland-compatible driver.
- Consider accessibility-driven input as a higher-integrity option than pointer
  automation in some cases.
- Keep the current desktop-coexistence policy intact as new drivers are added.

## 5. CLI and UX

- Improve `--help` descriptions so command intent is clearer without reading the
  docs.
- Add friendlier summaries for `targets` and `scenarios`.
- Decide whether the legacy rust-chatbot-specific commands should remain
  permanently or become thin conveniences over `run`.
- Add an inspection command for existing reports if it proves useful.

## 6. Packaging

- Decide whether `auto-ui` should stay repo-local or be published as an
  installable tool.
- Add release packaging guidance.
- Add pinned dependency/setup guidance for Linux hosts that run the tool
  regularly.

## 7. Documentation

- Keep [README.md](/home/m/git/auto-ui/README.md) focused on quick start.
- Keep [DESIGN.md](/home/m/git/auto-ui/DESIGN.md) focused on architecture.
- Keep [examples](/home/m/git/auto-ui/examples) aligned with the actual shipped
  CLI surface.

None of the above are migration blockers. They are the next layer of maturity
work after the migration.
