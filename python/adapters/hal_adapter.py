"""Optional HAL backends; a missing vendor SDK is an explicit runtime error.

JSON protocol is embedded in the Windows binary. SDR/audio transfers use
normalized full-scale samples, never volts without a calibration profile.
"""

import contextlib
import json
import sys


class Adapter:
    def __init__(self, settings, source):
        import numpy as np

        self.np = np
        self.c = settings
        self.source = source
        self.backend = settings["io_backend"]
        self.offset = 0
        self.index = 0
        self.caps = dict(
            clocks=["internal"],
            triggers=["immediate"],
            hardware_timestamps=False,
            ptp=False,
            gpsdo=False,
            max_channels=1,
        )
        c = settings
        if c["mimo_channels"] != 1 or c["trigger"] != "immediate":
            raise ValueError("Ce pilote supporte un canal, trigger immediate")
        if self.backend not in ("SOAPY", "UHD") and c["clock"] != "internal":
            raise ValueError("Horloge non disponible dans ce pilote")
        if self.backend == "SOAPY":
            import SoapySDR

            self.sdk = SoapySDR
            self.device = SoapySDR.Device(c["endpoint"])
            self.direction = SoapySDR.SOAPY_SDR_RX if source else SoapySDR.SOAPY_SDR_TX
            clocks = list(self.device.listClockSources())
            if clocks:
                self.caps["clocks"] = clocks
                if c["clock"] not in clocks:
                    raise ValueError("Clock source non disponible")
                self.device.setClockSource(c["clock"])
            self.device.setSampleRate(self.direction, 0, c["rate"])
            self.device.setFrequency(self.direction, 0, c["center_hz"])
            self.stream = self.device.setupStream(self.direction, SoapySDR.SOAPY_SDR_CF32, [0])
            status = self.device.activateStream(self.stream)
            if status < 0:
                raise OSError(f"Soapy activateStream status {status}")
            c["rate"] = self.device.getSampleRate(self.direction, 0)
            c["center_hz"] = self.device.getFrequency(self.direction, 0)
            self.caps["hardware_timestamps"] = bool(self.device.hasHardwareTime())
        elif self.backend == "UHD":
            import uhd

            self.sdk = uhd
            self.device = uhd.usrp.MultiUSRP(c["endpoint"])
            self.caps["clocks"] = list(self.device.get_clock_sources(0))
            if c["clock"] not in self.caps["clocks"]:
                raise ValueError("UHD clock non disponible")
            self.device.set_clock_source(c["clock"])
            stream_args = uhd.usrp.StreamArgs("fc32", "sc16")
            stream_args.channels = [0]
            if source:
                self.device.set_rx_rate(c["rate"])
                self.device.set_rx_freq(uhd.types.TuneRequest(c["center_hz"]), 0)
                c["rate"] = self.device.get_rx_rate(0)
                c["center_hz"] = self.device.get_rx_freq(0)
                self.stream = self.device.get_rx_stream(stream_args)
                command = uhd.types.StreamCMD(uhd.types.StreamMode.start_cont)
                command.stream_now = True
                self.stream.issue_stream_cmd(command)
            else:
                self.device.set_tx_rate(c["rate"])
                self.device.set_tx_freq(uhd.types.TuneRequest(c["center_hz"]), 0)
                c["rate"] = self.device.get_tx_rate(0)
                c["center_hz"] = self.device.get_tx_freq(0)
                self.stream = self.device.get_tx_stream(stream_args)
            self.caps["hardware_timestamps"] = source
        elif self.backend == "IIO":
            import adi

            self.device = adi.ad9361(uri=c["endpoint"])
            self.device.sample_rate = int(c["rate"])
            self.device.rx_buffer_size = c["samples"]
            self.device.rx_enabled_channels = [0]
            self.device.tx_enabled_channels = [0]
            self.device.rx_lo = int(c["center_hz"])
            self.device.tx_lo = int(c["center_hz"])
        elif self.backend == "AUDIO":
            import sounddevice as sd

            cls = sd.InputStream if source else sd.OutputStream
            self.stream = cls(
                samplerate=c["rate"],
                channels=2,
                dtype="float32",
                device=None if not c["endpoint"] else c["endpoint"],
            )
            self.stream.start()
        elif self.backend == "ZMQ":
            import zmq

            self.context = zmq.Context()
            self.socket = self.context.socket(zmq.SUB if source else zmq.PUSH)
            self.socket.setsockopt(zmq.LINGER, 0)
            self.socket.setsockopt(zmq.RCVTIMEO, c["timeout_ms"])
            self.socket.setsockopt(zmq.SNDTIMEO, c["timeout_ms"])
            self.socket.setsockopt(zmq.MAXMSGSIZE, 4_000_000)
            if source:
                self.socket.setsockopt(zmq.SUBSCRIBE, b"")
            self.socket.connect(c["endpoint"])
        elif self.backend == "HDF5":
            import h5py

            self.file = h5py.File(c["endpoint"], "r" if source else "x")
            if source:
                if self.file.attrs.get("rfworkbench_version") != 1:
                    raise ValueError("Schéma HDF5 non supporté")
                self.keys = sorted(self.file.keys())
            else:
                self.file.attrs["rfworkbench_version"] = 1
        elif self.backend == "PARQUET":
            import pyarrow as pa
            import pyarrow.parquet as pq

            self.pa = pa
            self.pq = pq
            if source:
                self.file = pq.ParquetFile(c["endpoint"])
                metadata = self.file.schema_arrow.metadata or {}
                if metadata.get(b"rfworkbench_version") != b"1":
                    raise ValueError("Schéma Parquet non supporté")
                self.rows = iter(self.file.iter_batches(batch_size=1))
            else:
                self.file = None
                self.schema = pa.schema(
                    [
                        ("metadata", pa.string()),
                        ("re", pa.list_(pa.float64())),
                        ("im", pa.list_(pa.float64())),
                    ],
                    metadata={b"rfworkbench_version": b"1"},
                )
                # Protect recordings: the native RAW path is the explicitly
                # overwriteable format; optional storage refuses existing files.
                import os

                if os.path.exists(c["endpoint"]):
                    raise ValueError("Le fichier Parquet existe déjà")
        else:
            raise ValueError("Backend HAL non supporté")
        if c["clock"] not in self.caps["clocks"]:
            raise ValueError("Horloge non disponible dans ce pilote")

    def frame(self, samples, epoch=None, discontinuity=False):
        f = dict(
            samples=[dict(re=float(z.real), im=float(z.imag)) for z in samples],
            sample_rate=self.c["rate"],
            center_hz=self.c["center_hz"],
            unit="Fs",
            simulated=False,
            time=dict(
                first_sample=self.offset,
                epoch_ns=epoch,
                clock_domain=self.backend,
                discontinuity=discontinuity,
            ),
        )
        self.offset += len(samples)
        return f

    def read(self):
        n, c, np = self.c["samples"], self.c, self.np
        if self.backend == "ZMQ":
            return self.socket.recv_json()
        if self.backend == "HDF5":
            if self.index >= len(self.keys):
                raise EOFError("Fin HDF5")
            g = self.file[self.keys[self.index]]
            f = json.loads(g.attrs["metadata"])
            samples = g["samples"][:]
            f["samples"] = [dict(re=float(z[0]), im=float(z[1])) for z in samples]
            self.index += 1
            return f
        if self.backend == "PARQUET":
            row = next(self.rows).to_pylist()[0]
            f = json.loads(row["metadata"])
            f["samples"] = [dict(re=r, im=i) for r, i in zip(row["re"], row["im"], strict=True)]
            return f
        if self.backend == "SOAPY":
            samples = np.empty(n, dtype=np.complex64)
            result = self.device.readStream(
                self.stream, [samples], n, timeoutUs=c["timeout_ms"] * 1000
            )
            if result.ret <= 0:
                raise OSError(f"Soapy readStream status {result.ret}")
            epoch = result.timeNs if result.flags & self.sdk.SOAPY_SDR_HAS_TIME else None
            return self.frame(samples[: result.ret], epoch)
        if self.backend == "UHD":
            samples = np.empty((1, n), dtype=np.complex64)
            metadata = self.sdk.types.RXMetadata()
            count = self.stream.recv(samples, metadata, c["timeout_ms"] / 1000)
            if metadata.error_code != self.sdk.types.RXMetadataErrorCode.none or count == 0:
                raise OSError(f"UHD receive {metadata.strerror()}")
            return self.frame(samples[0, :count], round(metadata.time_spec.get_real_secs() * 1e9))
        if self.backend == "IIO":
            return self.frame(np.asarray(self.device.rx()) / c.get("rx_scale", 2048.0))
        if self.backend == "AUDIO":
            data, overflow = self.stream.read(n)
            return self.frame(data[:, 0] + 1j * data[:, 1], discontinuity=overflow)
        raise ValueError("Lecture non disponible")

    def write(self, frame):
        np = self.np
        samples = np.asarray(
            [complex(z["re"], z["im"]) for z in frame["samples"]], dtype=np.complex64
        )
        if self.backend == "ZMQ":
            self.socket.send_json(frame)
        elif self.backend in ("HDF5", "PARQUET"):
            metadata = dict(frame, samples=[])
            if self.backend == "HDF5":
                g = self.file.create_group(f"{self.index:012d}")
                g.attrs["metadata"] = json.dumps(metadata, allow_nan=False)
                g.create_dataset("samples", data=np.column_stack((samples.real, samples.imag)))
                self.file.flush()
            else:
                table = self.pa.Table.from_pylist(
                    [
                        dict(
                            metadata=json.dumps(metadata, allow_nan=False),
                            re=samples.real.tolist(),
                            im=samples.imag.tolist(),
                        )
                    ],
                    schema=self.schema,
                )
                if self.file is None:
                    self.file = self.pq.ParquetWriter(self.c["endpoint"], self.schema)
                self.file.write_table(table)
            self.index += 1
        else:
            if frame["unit"] != "Fs" or frame["sample_rate"] != self.c["rate"]:
                raise ValueError("Sortie SDR/audio requiert FS et la cadence configurée")
            if np.max(np.abs(samples)) > 1:
                raise ValueError("Sortie FS au-delà de 1 : atténuer/calibrer explicitement")
            if self.backend == "SOAPY":
                result = self.device.writeStream(
                    self.stream, [samples], len(samples), timeoutUs=self.c["timeout_ms"] * 1000
                )
                if result.ret != len(samples):
                    raise OSError("Écriture Soapy partielle")
            elif self.backend == "UHD":
                metadata = self.sdk.types.TXMetadata()
                count = self.stream.send(
                    samples.reshape(1, -1), metadata, self.c["timeout_ms"] / 1000
                )
                if count != len(samples):
                    raise OSError("Écriture UHD partielle")
            elif self.backend == "IIO":
                self.device.tx(samples * self.c.get("tx_scale", 16384.0))
            elif self.backend == "AUDIO":
                if self.stream.write(np.column_stack((samples.real, samples.imag))):
                    raise OSError("Audio underflow")
            else:
                raise ValueError("Écriture non disponible")

    def close(self):
        if self.backend == "SOAPY":
            self.device.deactivateStream(self.stream)
            self.device.closeStream(self.stream)
        elif self.backend == "UHD" and self.source:
            self.stream.issue_stream_cmd(
                self.sdk.types.StreamCMD(self.sdk.types.StreamMode.stop_cont)
            )
        elif self.backend == "AUDIO":
            self.stream.stop()
            self.stream.close()
        elif self.backend in ("HDF5", "PARQUET") and self.file is not None:
            self.file.close()
        elif self.backend == "ZMQ":
            self.socket.close()
            self.context.term()


def main():
    adapter = None
    # Vendor modules sometimes print diagnostics to stdout. Reserve stdout
    # exclusively for protocol messages, redirect SDK output to stderr.
    protocol = sys.stdout
    try:
        for line in sys.stdin:
            try:
                if len(line) > 4_000_000:
                    raise ValueError("Message trop volumineux")
                request = json.loads(line)
                with contextlib.redirect_stdout(sys.stderr):
                    if request["op"] == "open":
                        adapter = Adapter(request["settings"], request["source"])
                        response = {"capabilities": adapter.caps}
                    elif request["op"] == "read":
                        response = {"frame": adapter.read()}
                    elif request["op"] == "write":
                        adapter.write(request["frame"])
                        response = {"ok": True}
                    elif request["op"] == "close":
                        adapter.close()
                        adapter = None
                        response = {"ok": True}
                    else:
                        raise ValueError("Opération inconnue")
                protocol.write(json.dumps(response, allow_nan=False) + "\n")
                protocol.flush()
            except Exception as exc:
                protocol.write(json.dumps({"error": f"{type(exc).__name__}: {exc}"}) + "\n")
                protocol.flush()
    finally:
        if adapter is not None:
            with contextlib.suppress(Exception):
                adapter.close()


if __name__ == "__main__":
    main()
