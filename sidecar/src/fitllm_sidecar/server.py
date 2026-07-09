"""Serve mode: newline-delimited JSON RPC over stdin/stdout.

Spawned by the Tauri shell as a long-lived sidecar. Reads one JSON request per
line on stdin and writes one JSON response per line on stdout. Progress events
for long operations are emitted out of band as extra stdout lines.

Protocol
--------
Request:  ``{"id": <int>, "method": <str>, "params": {...}}``
Response: ``{"id": <int>, "result": {...}}`` or ``{"id": <int>, "error": "<msg>"}``
Progress: ``{"event": "progress", "stage": <str>, "pct": <0-100>, "model": <str>}``

Methods: ``ping``, ``list_adapters``, ``list_models``, ``quick_benchmark``, ``chat``.
"""

from __future__ import annotations

import json
import sys
from typing import Any, Optional, TextIO

from .adapters import get_adapter, list_adapters as _list_adapters
from .benchmark import run_benchmark
from .sysmon import cpu_load, mem_used_mb

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
                system = body.get("system", "")

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

    print(f"fitllm-sidecar HTTP dev server: http://localhost:{port}")
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

        if method == "chat":
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
