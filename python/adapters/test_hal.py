"""Storage and network adapter integration tests, with optional SDK dependencies.

Run separately: python -m pytest adapters/test_hal.py (numpy/h5py/pyarrow/pyzmq).
Hardware SDK code requires a connected instrument and is not simulated here.
"""

import json
import threading

import pytest

from adapters.hal_adapter import Adapter


def settings(backend, path):
    return dict(
        io_backend=backend,
        endpoint=str(path),
        rate=48000.0,
        center_hz=100e6,
        samples=32,
        mimo_channels=1,
        clock="internal",
        trigger="immediate",
        timeout_ms=2000,
    )


def frame():
    return dict(
        samples=[dict(re=i / 16, im=-0.25) for i in range(32)],
        sample_rate=48000.0,
        center_hz=100e6,
        unit="Fs",
        simulated=True,
        time=dict(first_sample=123, epoch_ns=987654321, clock_domain="test", discontinuity=True),
    )


@pytest.mark.parametrize("backend,dependency", [("HDF5", "h5py"), ("PARQUET", "pyarrow")])
def test_storage_roundtrip_metadata_and_eof(tmp_path, backend, dependency):
    pytest.importorskip(dependency)
    c = settings(backend, tmp_path / "capture")
    original = frame()
    writer = Adapter(c, False)
    writer.write(original)
    writer.write(original)
    writer.close()
    reader = Adapter(c, True)
    try:
        assert reader.read() == original
        assert reader.read() == original
        with pytest.raises((EOFError, StopIteration)):
            reader.read()
    finally:
        reader.close()
    with pytest.raises((ValueError, FileExistsError)):
        Adapter(c, False)


def test_zmq_real_subscriber_retains_frame():
    zmq = pytest.importorskip("zmq")
    context = zmq.Context()
    publisher = context.socket(zmq.PUB)
    port = publisher.bind_to_random_port("tcp://127.0.0.1")
    subscriber = Adapter(settings("ZMQ", f"tcp://127.0.0.1:{port}"), True)
    original = frame()
    stop = threading.Event()

    def publish():
        while not stop.wait(0.02):
            publisher.send_json(original)

    thread = threading.Thread(target=publish)
    thread.start()
    try:
        assert subscriber.read() == original
    finally:
        stop.set()
        thread.join()
        subscriber.close()
        publisher.close(linger=0)
        context.term()


def test_capability_request_is_rejected_before_driver():
    c = settings("SOAPY", "")
    c["mimo_channels"] = 2
    with pytest.raises(ValueError, match="un canal"):
        Adapter(c, True)


def test_json_protocol_closes_storage_and_reports_errors(tmp_path):
    pytest.importorskip("h5py")
    import subprocess
    import sys
    from pathlib import Path

    requests = [
        dict(op="open", settings=settings("HDF5", tmp_path / "protocol.h5"), source=False),
        dict(op="write", frame=frame()),
        dict(op="close"),
        dict(op="bad"),
    ]
    result = subprocess.run(
        [sys.executable, "-u", str(Path(__file__).with_name("hal_adapter.py"))],
        input="".join(json.dumps(r) + "\n" for r in requests),
        text=True,
        capture_output=True,
        timeout=10,
        check=True,
    )
    replies = [json.loads(line) for line in result.stdout.splitlines()]
    assert replies[0]["capabilities"]["max_channels"] == 1
    assert replies[1] == {"ok": True}
    assert replies[2] == {"ok": True}
    assert "error" in replies[3]


def test_no_unsupported_clock_is_reported_as_ready(tmp_path):
    pytest.importorskip("h5py")
    c = settings("HDF5", tmp_path / "clock.h5")
    c["clock"] = "PTP"
    with pytest.raises(ValueError, match="Horloge"):
        Adapter(c, False)
