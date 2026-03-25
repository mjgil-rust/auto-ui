#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import pathlib
import re
import subprocess
import time
from dataclasses import dataclass
from typing import Any


REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
FIELD_RE = re.compile(r"(\w+)=((?:\"[^\"]*\")|(?:\S+))")


@dataclass
class SessionEntry:
    session_id: str
    name: str
    updated_at: str
    message_count: int


@dataclass
class WindowGeometry:
    x: int
    y: int
    width: int
    height: int


def run(
    cmd: list[str],
    *,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, check=check, text=True, capture_output=True, env=env)


def log_line(message: str, progress_path: pathlib.Path | None = None) -> None:
    print(message, flush=True)
    if progress_path is not None:
        with progress_path.open("a", encoding="utf-8") as fh:
            fh.write(message)
            fh.write("\n")


def resolve_app_root(raw_path: str | None = None) -> pathlib.Path:
    if raw_path:
        return pathlib.Path(raw_path).expanduser().resolve()

    for env_name in ("RUST_CHATBOT_APP_ROOT", "RUST_CHATBOT_ROOT"):
        value = os.environ.get(env_name)
        if value:
            return pathlib.Path(value).expanduser().resolve()

    sibling = REPO_ROOT.parent / "rust-chatbot"
    if sibling.exists():
        return sibling.resolve()

    raise SystemExit(
        "Could not resolve the rust-chatbot app root. Pass --app-root or set "
        "RUST_CHATBOT_APP_ROOT."
    )


def provider_title(provider: str, instance: int | None, app_root: pathlib.Path) -> str:
    base = {
        "claude": "Claude Rust Chatbot",
        "codex": "Codex Rust Chatbot",
        "gemini": "Gemini Rust Chatbot",
    }[provider]
    dir_name = app_root.name
    if dir_name:
        return f"{base} ~ /{dir_name}"
    return base


def provider_data_dir(provider: str) -> pathlib.Path:
    home = pathlib.Path.home()
    return {
        "claude": home / ".claude-desktop",
        "codex": home / ".codex-desktop",
        "gemini": home / ".gemini-desktop",
    }[provider]


def rust_chatbot_log_dir() -> pathlib.Path:
    xdg_state = os.environ.get("XDG_STATE_HOME")
    if xdg_state:
        return pathlib.Path(xdg_state) / "rust-chatbot"
    return pathlib.Path.home() / ".local" / "state" / "rust-chatbot"


def newest_trace_log() -> pathlib.Path:
    log_dir = rust_chatbot_log_dir()
    candidates = sorted(log_dir.glob("rust-chatbot.log.*"))
    if not candidates:
        raise SystemExit(f"No rust-chatbot tracing log found in {log_dir}")
    return candidates[-1]


def load_sessions(
    provider: str,
    max_sessions: int,
    include_hidden: bool,
    *,
    default_session_names: list[str] | None = None,
) -> list[SessionEntry]:
    metadata_path = provider_data_dir(provider) / "sessions.json"
    payload = json.loads(metadata_path.read_text(encoding="utf-8"))
    sessions = []
    seen_names: set[str] = set()
    for raw in payload.get("sessions", {}).values():
        if not include_hidden and raw.get("hidden"):
            continue
        if raw.get("message_count", 0) <= 0:
            continue
        name = raw["name"]
        if name in seen_names:
            continue
        seen_names.add(name)
        sessions.append(
            SessionEntry(
                session_id=raw["id"],
                name=name,
                updated_at=raw.get("updated_at", ""),
                message_count=int(raw.get("message_count", 0)),
            )
        )
    if default_session_names:
        sessions_by_name = {session.name: session for session in sessions}
        missing_names = [name for name in default_session_names if name not in sessions_by_name]
        if missing_names:
            missing = ", ".join(missing_names)
            raise SystemExit(f"Default auto-ui sessions were not found: {missing}")
        return [sessions_by_name[name] for name in default_session_names]

    sessions.sort(key=lambda item: item.updated_at, reverse=True)
    return sessions[:max_sessions]


def load_session_by_id(provider: str, session_id: str) -> SessionEntry:
    metadata_path = provider_data_dir(provider) / "sessions.json"
    payload = json.loads(metadata_path.read_text(encoding="utf-8"))
    raw = payload.get("sessions", {}).get(session_id)
    if raw is None:
        raise SystemExit(f"Session {session_id!r} was not found for provider {provider!r}.")
    return SessionEntry(
        session_id=raw["id"],
        name=raw["name"],
        updated_at=raw.get("updated_at", ""),
        message_count=int(raw.get("message_count", 0)),
    )


def require_release_binaries(
    app_root: pathlib.Path,
    required_binaries: tuple[str, ...] = ("chatbot-ctl", "rust-chatbot"),
) -> None:
    missing = [
        app_root / "target" / "release" / binary
        for binary in required_binaries
        if not (app_root / "target" / "release" / binary).exists()
    ]
    if not missing:
        return
    missing_text = ", ".join(str(path) for path in missing)
    raise SystemExit(
        "Missing release binaries required by this script: "
        f"{missing_text}. Build them first with the lightweight build path."
    )


def chatbot_ctl_path(app_root: pathlib.Path) -> pathlib.Path:
    return app_root / "target" / "release" / "chatbot-ctl"


def list_chatbot_pids(app_root: pathlib.Path) -> set[int]:
    require_release_binaries(app_root, ("chatbot-ctl",))
    proc = run([str(chatbot_ctl_path(app_root)), "pids"], check=False)
    pids: set[int] = set()
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            pids.add(int(line))
        except ValueError:
            continue
    return pids


def launch_window(
    app_root: pathlib.Path,
    provider: str,
    instance: int | None,
    start_session_id: str | None = None,
) -> None:
    require_release_binaries(app_root)
    env = os.environ.copy()
    env["RUST_CHATBOT_AUTO_UI_DEBUG"] = "1"
    if start_session_id:
        env["RUST_CHATBOT_START_SESSION_ID"] = start_session_id
    cmd = [str(chatbot_ctl_path(app_root)), "launch", "--provider", provider]
    if instance is not None:
        cmd.extend(["--instance", str(instance)])
    subprocess.run(cmd, cwd=app_root, env=env, check=True)


def find_interaction_window_ids(title_substring: str) -> list[str]:
    proc = run(["xdotool", "search", "--name", re.escape(title_substring)], check=False)
    return [line.strip() for line in proc.stdout.splitlines() if line.strip()]


def find_interaction_window_ids_for_pid(pid: int) -> list[str]:
    proc = run(["xdotool", "search", "--pid", str(pid)], check=False)
    return [line.strip() for line in proc.stdout.splitlines() if line.strip()]


def find_window_ids(title_substring: str) -> list[str]:
    proc = run(["wmctrl", "-l"], check=False)
    window_ids: list[str] = []
    for line in proc.stdout.splitlines():
        parts = line.split(None, 3)
        if len(parts) < 4:
            continue
        hex_id = parts[0]
        title = parts[3]
        if title_substring not in title:
            continue
        window_ids.append(str(int(hex_id, 16)))
    return window_ids


def find_window_ids_for_pid(pid: int) -> list[str]:
    proc = run(["wmctrl", "-lp"], check=False)
    window_ids: list[str] = []
    for line in proc.stdout.splitlines():
        parts = line.split(None, 4)
        if len(parts) < 4:
            continue
        if parts[2] != str(pid):
            continue
        window_ids.append(str(int(parts[0], 16)))
    return window_ids


def find_interaction_window_id(title_substring: str, timeout_s: float) -> str | None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        ids = find_interaction_window_ids(title_substring)
        if ids:
            return ids[-1]
        time.sleep(0.2)
    return None


def find_interaction_window_id_for_pid(pid: int, timeout_s: float) -> str | None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        ids = find_interaction_window_ids_for_pid(pid)
        if ids:
            return ids[-1]
        time.sleep(0.2)
    return None


def find_window_id(title_substring: str, timeout_s: float) -> str | None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        ids = find_window_ids(title_substring)
        if ids:
            return ids[-1]
        time.sleep(0.2)
    return None


def find_window_id_for_pid(pid: int, timeout_s: float) -> str | None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        ids = find_window_ids_for_pid(pid)
        if ids:
            return ids[-1]
        time.sleep(0.2)
    return None


def wait_for_new_pid(app_root: pathlib.Path, before_pids: set[int], timeout_s: float) -> int | None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        current = list_chatbot_pids(app_root)
        new_pids = sorted(current - before_pids)
        if new_pids:
            return new_pids[-1]
        time.sleep(0.2)
    return None


def stop_chatbot_pid(app_root: pathlib.Path, pid: int) -> None:
    run([str(chatbot_ctl_path(app_root)), "stop", str(pid)], check=False)


def wait_for_pid_exit(app_root: pathlib.Path, pid: int, timeout_s: float) -> None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        if pid not in list_chatbot_pids(app_root):
            return
        time.sleep(0.2)


def launch_targeted_session_window(
    *,
    app_root: pathlib.Path,
    provider: str,
    instance: int | None,
    title: str,
    session_id: str,
    keep_front: bool,
    window_timeout: float,
    progress_path: pathlib.Path | None,
) -> tuple[int, str]:
    existing_pids = list_chatbot_pids(app_root)
    existing_window_ids = set(find_window_ids(title))
    log_line(
        f"launching fresh {title} with RUST_CHATBOT_AUTO_UI_DEBUG=1 session_id={session_id}",
        progress_path,
    )
    launch_window(app_root, provider, instance, session_id)
    launched_pid = wait_for_new_pid(app_root, existing_pids, window_timeout)
    if launched_pid is None:
        raise SystemExit(f"Could not detect a newly launched PID for {title!r}.")
    log_line(f"detected launched_pid={launched_pid}", progress_path)
    window_id = find_window_id_for_pid(launched_pid, window_timeout)
    if window_id is None:
        window_id = wait_for_new_window_id(title, existing_window_ids, window_timeout)
    if window_id is None:
        raise SystemExit(f"Could not find a new window matching {title!r} after launch.")
    if not keep_front:
        lower_window(window_id)
    log_line(f"using window_id={window_id}", progress_path)
    return launched_pid, window_id


def wait_for_new_interaction_window_id(
    title_substring: str,
    before_ids: set[str],
    timeout_s: float,
) -> str | None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        ids = find_interaction_window_ids(title_substring)
        for window_id in ids:
            if window_id not in before_ids:
                return window_id
        time.sleep(0.2)
    return None


def wait_for_new_window_id(title_substring: str, before_ids: set[str], timeout_s: float) -> str | None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        ids = find_window_ids(title_substring)
        for window_id in ids:
            if window_id not in before_ids:
                return window_id
        time.sleep(0.2)
    return None


def get_window_geometry(window_id: str) -> WindowGeometry:
    proc = run(["xdotool", "getwindowgeometry", "--shell", window_id])
    values: dict[str, int] = {}
    for line in proc.stdout.splitlines():
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        if key in {"X", "Y", "WIDTH", "HEIGHT"}:
            values[key] = int(value)
    return WindowGeometry(
        x=values["X"],
        y=values["Y"],
        width=values["WIDTH"],
        height=values["HEIGHT"],
    )


def activate_window(window_id: str) -> None:
    run(["xdotool", "windowactivate", "--sync", window_id])


def lower_window(window_id: str) -> None:
    run(["wmctrl", "-i", "-r", window_id, "-b", "remove,above"], check=False)
    run(["wmctrl", "-i", "-r", window_id, "-b", "add,below"], check=False)


def resize_window(window_id: str, width: int, height: int) -> WindowGeometry:
    run(["xdotool", "windowsize", "--sync", window_id, str(width), str(height)])
    return get_window_geometry(window_id)


def press_key(window_id: str, key: str) -> None:
    run(["xdotool", "key", "--window", window_id, "--clearmodifiers", key])


def clear_search(window_id: str) -> None:
    press_key(window_id, "ctrl+a")
    press_key(window_id, "BackSpace")


def type_text(window_id: str, text: str) -> None:
    run(["xdotool", "type", "--window", window_id, "--delay", "1", "--clearmodifiers", text])


def select_session(window_id: str, session_name: str) -> None:
    press_key(window_id, "ctrl+f")
    time.sleep(0.1)
    clear_search(window_id)
    type_text(window_id, session_name)
    time.sleep(0.1)
    press_key(window_id, "Return")


def read_new_lines(log_path: pathlib.Path, offset: int) -> tuple[int, list[str]]:
    with log_path.open("r", encoding="utf-8", errors="replace") as fh:
        fh.seek(offset)
        data = fh.read()
        new_offset = fh.tell()
    return new_offset, data.splitlines()


def parse_trace_fields(line: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for key, value in FIELD_RE.findall(line):
        if value.startswith('"') and value.endswith('"'):
            value = value[1:-1]
        fields[key] = value
    return fields


def wait_for_trace_bundle(
    log_path: pathlib.Path,
    offset: int,
    *,
    session_id: str,
    timeout_s: float,
    quiet_s: float = 0.5,
) -> tuple[int, dict[str, str], list[dict[str, str]]]:
    deadline = time.time() + timeout_s
    quiet_deadline: float | None = None
    current_offset = offset
    last_ui_match: dict[str, str] | None = None
    code_blocks: list[dict[str, str]] = []

    while time.time() < deadline:
        current_offset, lines = read_new_lines(log_path, current_offset)
        saw_new_relevant_line = False
        for line in lines:
            if "ui_auto_debug_code_block" in line:
                code_blocks.append(parse_trace_fields(line))
                saw_new_relevant_line = True
                continue
            if "ui_auto_debug" in line and f"session_id={session_id}" in line:
                last_ui_match = parse_trace_fields(line)
                saw_new_relevant_line = True

        if last_ui_match is not None:
            if saw_new_relevant_line:
                quiet_deadline = time.time() + quiet_s
            elif quiet_deadline is not None and time.time() >= quiet_deadline:
                return current_offset, last_ui_match, code_blocks

        time.sleep(0.15)

    if last_ui_match is not None:
        return current_offset, last_ui_match, code_blocks
    raise TimeoutError(f"No ui_auto_debug trace observed for session {session_id}")


def capture_window_screenshot(window_id: str, path: pathlib.Path) -> None:
    run(["import", "-window", window_id, str(path)])


def crop_metric(image_path: pathlib.Path, x: int, y: int, width: int, height: int) -> dict[str, float]:
    geometry = f"{width}x{height}+{x}+{y}"
    stddev = run(
        [
            "convert",
            str(image_path),
            "-crop",
            geometry,
            "+repage",
            "-colorspace",
            "Gray",
            "-format",
            "%[fx:standard_deviation]",
            "info:",
        ]
    ).stdout.strip()
    colors = run(
        [
            "identify",
            "-format",
            "%k",
            str(image_path) + f"[{geometry}]",
        ]
    ).stdout.strip()
    return {
        "stddev": float(stddev or 0.0),
        "colors": float(colors or 0.0),
    }


def image_size(image_path: pathlib.Path) -> tuple[int, int]:
    output = run(["identify", "-format", "%w %h", str(image_path)]).stdout.strip()
    width_str, height_str = output.split()
    return int(width_str), int(height_str)


def heuristic_text_visible(metric: dict[str, float]) -> bool:
    return metric["stddev"] >= 0.01 or metric["colors"] >= 16.0


def parse_widths(raw: str) -> list[int]:
    return [int(part.strip()) for part in raw.split(",") if part.strip()]


def build_output_dir(output_dir: str | None, prefix: str) -> pathlib.Path:
    if output_dir:
        path = pathlib.Path(output_dir).expanduser()
    else:
        stamp = time.strftime("%Y%m%d-%H%M%S")
        path = REPO_ROOT / "tmp" / f"{prefix}-{stamp}"
    path.mkdir(parents=True, exist_ok=True)
    return path.resolve()
