"""CLI entry point: ``python -m llamachat_sidecar <command> [args]``.

Commands
--------
- ``list-adapters``                    → ``{"adapters":[{name,available,detail}]}``
- ``list-models --adapter <name>``     → ``{"models":[{name,size_mb}]}``
- ``benchmark --adapter <name> --model <tag> [--tier quick|balanced|full]``
                                       → one ``BenchmarkResult`` JSON object
- ``serve``                            → stdin/stdout JSON-line RPC loop

All commands print JSON to stdout and exit 0 on a handled error (the error is
carried in the JSON payload) so callers always get parseable output.
"""

from __future__ import annotations

import argparse
import json
import sys

from .adapters import get_adapter, list_adapters
from .adapters.base import empty_result
from .benchmark import run_benchmark
from .server import serve


def _print(obj: dict) -> None:
    print(json.dumps(obj))


def cmd_list_adapters(_args) -> int:
    _print({"adapters": list_adapters()})
    return 0


def cmd_list_models(args) -> int:
    adapter = get_adapter(args.adapter)
    if adapter is None:
        _print({"models": [], "error": f"unknown adapter '{args.adapter}'"})
        return 0
    _print({"models": adapter.list_models()})
    return 0


def cmd_benchmark(args) -> int:
    adapter = get_adapter(args.adapter)
    if adapter is None:
        _print(
            empty_result(
                args.model, args.adapter, args.tier, f"unknown adapter '{args.adapter}'"
            )
        )
        return 0
    result = run_benchmark(adapter, args.model, tier=args.tier)
    _print(result)
    return 0


def cmd_serve(_args) -> int:
    serve()
    return 0


def cmd_dev_server(args) -> int:
    from .server import _start_http_server
    _start_http_server(port=args.port)
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="llamachat_sidecar",
        description="LlamaChat benchmark sidecar — talks to local LLM runtimes (Ollama).",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    p_la = sub.add_parser("list-adapters", help="list available runtime adapters")
    p_la.set_defaults(func=cmd_list_adapters)

    p_lm = sub.add_parser("list-models", help="list locally available models")
    p_lm.add_argument("--adapter", default="ollama", help="adapter name (default: ollama)")
    p_lm.set_defaults(func=cmd_list_models)

    p_bench = sub.add_parser("benchmark", help="run a benchmark for one model")
    p_bench.add_argument("--adapter", default="ollama", help="adapter name (default: ollama)")
    p_bench.add_argument("--model", required=True, help="model tag, e.g. llama3.2:1b")
    p_bench.add_argument(
        "--tier",
        default="quick",
        choices=["quick", "balanced", "full"],
        help="benchmark intensity: quick (lightweight), balanced (a few "
        "minutes), full (deepest, most accurate). Default: quick.",
    )
    p_bench.set_defaults(func=cmd_benchmark)

    p_serve = sub.add_parser("serve", help="run the stdin/stdout JSON-line RPC loop")
    p_serve.set_defaults(func=cmd_serve)

    p_dev = sub.add_parser("dev-server", help="start HTTP dev server for UI development")
    p_dev.add_argument("--port", type=int, default=9199, help="HTTP port (default: 9199)")
    p_dev.set_defaults(func=cmd_dev_server)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv if argv is not None else sys.argv[1:])
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
