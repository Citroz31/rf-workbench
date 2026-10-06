from __future__ import annotations

import io
import json
import runpy
import sys
from typing import Any
from unittest.mock import patch

import pytest

from rfworkbench import runner

TRACE: dict[str, Any] = {
    "frequency_hz": [1e9, 2e9],
    "amplitude_dbm": [-20.0, -10.0],
    "simulated": True,
}


def invoke(source: str, responses: list[dict[str, Any]] | None = None) -> list[dict[str, Any]]:
    request = json.dumps({"source": source, "trace": TRACE, "hardware": False}) + "\n"
    request += "".join(json.dumps(r) + "\n" for r in responses or [])
    output = io.StringIO()
    with (
        patch.object(sys, "stdin", io.StringIO(request)),
        patch.object(sys, "__stdout__", output),
        patch.object(sys, "stderr", io.StringIO()),
    ):
        runner.main()
    return [json.loads(line) for line in output.getvalue().splitlines()]


def test_trace_compensation() -> None:
    messages = invoke(
        "output = dict(trace)\n"
        "output['amplitude_dbm'] = [level + 0.5 for level in trace['amplitude_dbm']]"
    )
    assert messages[0]["trace"]["amplitude_dbm"] == [-19.5, -9.5]
    assert messages[0]["trace"]["simulated"] is True


def test_print_does_not_corrupt_json_protocol() -> None:
    messages = invoke("print('Measurement status')\noutput = trace")
    assert len(messages) == 1
    assert messages[0]["op"] == "result"


def test_python_instrument_calls_are_delegated_to_rust() -> None:
    messages = invoke(
        "rm = ResourceManager()\n"
        "device = rm.open_resource('SIM::RF::INSTR')\n"
        "device.write(':POW -10')\n"
        "assert device.query('*IDN?') == 'ACME,RF,1,1'\n"
        "output = trace",
        [{"ok": True, "value": ""}, {"ok": True, "value": "ACME,RF,1,1"}],
    )
    assert [m["op"] for m in messages] == ["write", "query", "result"]
    assert messages[0]["command"] == ":POW -10"
    assert messages[1]["resource"] == "SIM::RF::INSTR"


@pytest.mark.parametrize("source", ["raise ValueError('bad calibration')", "output = missing"])
def test_script_errors_are_reported(source: str) -> None:
    message = invoke(source)[0]
    assert message["op"] == "error"
    assert "<RF Workbench>" in message["message"]


def test_backend_error_is_reported_to_script() -> None:
    messages = invoke(
        "ResourceManager().open_resource('USB0::1::INSTR').query('*IDN?')",
        [{"ok": False, "error": "VISA runtime unavailable"}],
    )
    assert messages[-1]["op"] == "error"
    assert "VISA runtime unavailable" in messages[-1]["message"]


def test_non_finite_result_is_not_encoded_as_measurement() -> None:
    message = invoke("output = dict(trace)\noutput['amplitude_dbm'] = [float('nan'), 1]")[0]
    assert message["op"] == "error"


def test_missing_output_is_an_explicit_error() -> None:
    message = invoke("x = 1")[0]
    assert message["op"] == "error"
    assert "output" in message["message"]


def test_cli_entrypoint_runs_protocol() -> None:
    output = io.StringIO()
    request = json.dumps({"source": "output = trace", "trace": TRACE}) + "\n"
    with patch.object(sys, "stdin", io.StringIO(request)), patch.object(sys, "__stdout__", output):
        runpy.run_path(str(runner.__file__), run_name="__main__")
    assert json.loads(output.getvalue())["op"] == "result"
