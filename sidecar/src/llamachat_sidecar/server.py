"""Serve mode: newline-delimited JSON RPC over stdin/stdout.

Spawned by the Tauri shell as a long-lived sidecar. Reads one JSON request per
line on stdin and writes one JSON response per line on stdout. Progress events
for long operations are emitted out of band as extra stdout lines.

Protocol
--------
Request:  ``{"id": <int>, "method": <str>, "params": {...}}``
Response: ``{"id": <int>, "result": {...}}`` or ``{"id": <int>, "error": "<msg>"}``
Progress: ``{"event": "progress", "stage": <str>, "pct": <0-100>, "model": <str>}``

Methods: ``ping``, ``list_adapters``, ``list_models``, ``quick_benchmark``, ``chat``, ``agent``.
"""

from __future__ import annotations

import json
import sys
from typing import Any, Optional, TextIO

from .adapters import get_adapter, list_adapters as _list_adapters
from .benchmark import run_benchmark
from .sysmon import cpu_load, mem_used_mb

# ── Agent tool system prompt ─────────────────────────────────────
# Mirrors ToolRegistry::system_prompt() in llamachat-core (crates/llamachat-core/
# src/tools/mod.rs). Kept in sync so the sidecar's /agent endpoint drives the
# same shell/file/process/desktop tools the Rust agent loop understands.
TOOL_SYSTEM_PROMPT = """You have access to the following tools. To use a tool, output a JSON object with "tool" and "args":

## shell
Run a shell command and return its output. Use for: listing files, checking system state, running builds, git operations. Do NOT use for: infinite loops, interactive commands, or commands that modify system configuration without user approval.
Parameters:
  - command: string (required) The shell command to execute.
  - cwd: string (optional) Working directory for the command.

## file
Read or write files on the filesystem. Use 'read' to view a file's contents, 'write' to create or overwrite a file, 'edit' for targeted text replacements.
Parameters:
  - action: string (required) One of: read, write, edit
  - path: string (required) Absolute or relative file path.
  - content: string (optional) Content to write (required for write action).
  - old_text: string (optional) Text to find and replace (required for edit action).
  - new_text: string (optional) Replacement text (required for edit action).

## process
List running processes or manage background tasks. Use 'list' to see what's running, 'spawn' to start a background command, 'kill' to stop a process by PID.
Parameters:
  - action: string (required) One of: list, spawn, kill
  - command: string (optional) Command to run (required for spawn).
  - pid: number (optional) Process ID to kill (required for kill).

## desktop
Take screenshots of the desktop to see what's on screen. Use to inspect UI, read error messages, or verify visual state before interacting. Returns the file path of the screenshot.
Parameters:
  - action: string (required) Action: 'screenshot' to capture the full screen
  - path: string (optional) Where to save the screenshot (defaults to temp file).

Respond with tool calls like:
{"tool": "shell", "args": {"command": "ls -la"}}
You can use multiple tools in sequence. After tool results, continue your response.
"""


def _build_agent_system(user_system: str = "") -> str:
    """Prepend the tool instructions to any caller-supplied system prompt."""
    if user_system:
        return f"{TOOL_SYSTEM_PROMPT}\n{user_system}"
    return TOOL_SYSTEM_PROMPT


# ── Chat tool system prompt ──────────────────────────────────────
# A compact tool description auto-injected into /chat when the caller
# supplies no system prompt, so an out-of-the-box chat session can still
# discover and drive the shell/file/process/desktop tools.
CHAT_TOOL_SYSTEM_PROMPT = """You have access to tools. To use a tool, output exactly:
{"tool": "<name>", "args": {...}}

Available tools:
- shell: Run shell commands. Args: {"command": "<cmd>", "cwd": "<optional dir>"}
- file: Read/write/edit files. Args: {"action": "read|write|edit", "path": "<path>", "content": "<optional>"}
- process: List/spawn/kill processes. Args: {"action": "list|spawn|kill", "command": "<optional>", "pid": <optional>}
- desktop: Take screenshots. Args: {"action": "screenshot", "path": "<optional path>"}

When you use a tool, the result will be shown. Then continue your response naturally."""


# ── Simple HTTP server for dev mode ──────────────────────────────

def _start_http_server(port: int = 9199) -> None:
    """Start a tiny HTTP server that the UI can call during development.
    Only used outside Tauri — the real path is stdin/stdout serve mode."""
    from http.server import HTTPServer, BaseHTTPRequestHandler

    class Handler(BaseHTTPRequestHandler):
        def do_OPTIONS(self):
            self.send_response(204)
            self._cors()
            self.end_headers()

        def do_POST(self):
            length = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(length)) if length else {}

            if self.path == "/chat":
                adapter = get_adapter(body.get("adapter", "ollama"))
                if not adapter:
                    self._json(400, {"error": "adapter not available"})
                    return
                model = body.get("model", "llama3.2:1b")
                messages = body.get("messages", [])
                # Auto-inject a tool-aware system prompt when the caller
                # supplies none, so a bare chat session can still use tools.
                system = body.get("system", "") or CHAT_TOOL_SYSTEM_PROMPT

                # Stream as SSE
                self.send_response(200)
                self._cors()
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                self.end_headers()

                full = []
                for token in adapter.chat(model, messages, system=system, stream=True):
                    full.append(token)
                    self.wfile.write(f"data: {json.dumps({'token': token})}\n\n".encode())
                    self.wfile.flush()
                self.wfile.write(f"data: {json.dumps({'done': True, 'content': ''.join(full)})}\n\n".encode())
            elif self.path == "/agent":
                # Same as /chat but injects the tool system prompt so the model
                # can emit {"tool": ..., "args": ...} calls the shell drives.
                adapter = get_adapter(body.get("adapter", "ollama"))
                if not adapter:
                    self._json(400, {"error": "adapter not available"})
                    return
                model = body.get("model", "llama3.2:3b")
                messages = body.get("messages", [])
                system = _build_agent_system(body.get("system", ""))

                self.send_response(200)
                self._cors()
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                self.end_headers()

                full = []
                for token in adapter.chat(model, messages, system=system, stream=True):
                    full.append(token)
                    self.wfile.write(f"data: {json.dumps({'token': token})}\n\n".encode())
                    self.wfile.flush()
                self.wfile.write(f"data: {json.dumps({'done': True, 'content': ''.join(full)})}\n\n".encode())
            elif self.path == "/tools":
                result = _list_adapters()
                self._json(200, {"adapters": result})
            else:
                self._json(404, {"error": "not found"})

        def _json(self, status: int, data: dict):
            self.send_response(status)
            self._cors()
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(data).encode())

        def _cors(self):
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Access-Control-Allow-Methods", "POST, OPTIONS")
            self.send_header("Access-Control-Allow-Headers", "Content-Type")

        def log_message(self, *args):
            pass  # quiet

    print(f"llamachat-sidecar HTTP dev server: http://localhost:{port}")
    HTTPServer(("0.0.0.0", port), Handler).serve_forever()


def _write(out: TextIO, obj: dict) -> None:
    """Write one JSON object as a single line and flush immediately."""
    out.write(json.dumps(obj) + "\n")
    out.flush()


def _emit_progress(out: TextIO, model: str, stage: str, pct: int) -> None:
    _write(
        out,
        {
            "event": "progress",
            "stage": stage,
            "pct": max(0, min(100, int(pct))),
            "model": model,
        },
    )


def handle_request(req: dict, out: TextIO) -> dict:
    """Dispatch a single request dict and return the response dict.

    Progress events (if any) are written to ``out`` as a side effect.
    """
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params") or {}

    if not isinstance(method, str):
        return {"id": req_id, "error": "missing or invalid 'method'"}

    try:
        if method == "ping":
            return {"id": req_id, "result": {"pong": True}}

        if method == "list_adapters":
            return {"id": req_id, "result": {"adapters": _list_adapters()}}

        if method == "list_models":
            name = params.get("adapter", "ollama")
            adapter = get_adapter(name)
            if adapter is None:
                return {"id": req_id, "error": f"unknown adapter '{name}'"}
            return {"id": req_id, "result": {"models": adapter.list_models()}}

        if method == "quick_benchmark":
            name = params.get("adapter", "ollama")
            model = params.get("model")
            if not model:
                return {"id": req_id, "error": "missing 'model' param"}
            adapter = get_adapter(name)
            if adapter is None:
                return {"id": req_id, "error": f"unknown adapter '{name}'"}

            def progress(stage: str, pct: int) -> None:
                _emit_progress(out, model, stage, pct)

            result = run_benchmark(adapter, model, tier="quick", progress=progress)
            return {"id": req_id, "result": result}

        if method in ("chat", "agent"):
            name = params.get("adapter", "ollama")
            model = params.get("model")
            messages = params.get("messages", [])
            system = params.get("system", "")

            if not model:
                return {"id": req_id, "error": "missing 'model' param"}
            if not messages:
                return {"id": req_id, "error": "missing 'messages' param"}

            adapter = get_adapter(name)
            if adapter is None:
                return {"id": req_id, "error": f"unknown adapter '{name}'"}

            # `agent` behaves like `chat` but injects the tool system prompt so
            # the model can emit tool calls the client executes.
            if method == "agent":
                system = _build_agent_system(system)

            # Stream tokens as progress events, collect full response
            full = []
            for token in adapter.chat(model, messages, system=system, stream=True):
                full.append(token)
                _write(out, {
                    "event": "token",
                    "token": token,
                    "id": req_id,
                })

            return {
                "id": req_id,
                "result": {
                    "model": model,
                    "adapter": name,
                    "content": "".join(full),
                    "done": True,
                },
            }

        return {"id": req_id, "error": f"unknown method '{method}'"}
    except Exception as exc:  # never let one bad request kill the loop
        return {"id": req_id, "error": f"{type(exc).__name__}: {exc}"}


def serve(stdin: Optional[TextIO] = None, stdout: Optional[TextIO] = None) -> None:
    """Run the stdin/stdout JSON-line RPC loop until EOF."""
    stdin = stdin or sys.stdin
    stdout = stdout or sys.stdout

    for line in stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except Exception as exc:
            _write(stdout, {"id": None, "error": f"invalid JSON: {exc}"})
            continue
        if not isinstance(req, dict):
            _write(stdout, {"id": None, "error": "request must be a JSON object"})
            continue
        response = handle_request(req, stdout)
        _write(stdout, response)
