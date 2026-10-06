"""Stdlib JSON-lines bridge, launched and supervised by the Rust worker.

User code runs as a regular local Python process, not a security sandbox.
"""

from __future__ import annotations

import contextlib
import json
import sys
import traceback
from typing import Any


class Instrument:
    def __init__(self, resource: str) -> None:
        self.resource = resource

    def _request(self, op: str, command: str) -> str:
        print(
            json.dumps({"op": op, "resource": self.resource, "command": command}),
            file=sys.__stdout__,
            flush=True,
        )
        response = json.loads(sys.stdin.readline())
        if not response["ok"]:
            raise RuntimeError(response["error"])
        return str(response["value"])

    def query(self, command: str) -> str:
        return self._request("query", command)

    def write(self, command: str) -> None:
        self._request("write", command)


class ResourceManager:
    def open_resource(self, resource: str) -> Instrument:
        return Instrument(resource)


def main() -> None:
    try:
        request = json.loads(sys.stdin.readline())
        namespace: dict[str, Any] = {
            "trace": request["trace"],
            "ResourceManager": ResourceManager,
            "__name__": "__rf_script__",
        }
        # print() in user code goes to stderr; protocol stdout stays clean.
        with contextlib.redirect_stdout(sys.stderr):
            exec(compile(request["source"], "<RF Workbench>", "exec"), namespace)
        print(
            json.dumps({"op": "result", "trace": namespace["output"]}, allow_nan=False),
            file=sys.__stdout__,
            flush=True,
        )
    except Exception:
        print(
            json.dumps({"op": "error", "message": traceback.format_exc(limit=5)}),
            file=sys.__stdout__,
            flush=True,
        )


if __name__ == "__main__":
    main()
