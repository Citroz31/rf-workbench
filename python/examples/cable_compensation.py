"""Paste into a Python block. Instrument I/O is delegated to Rust."""
rm = ResourceManager()
analyzer = rm.open_resource("SIM::RF::INSTR")
identity = analyzer.query("*IDN?")

output = dict(trace)
output["amplitude_dbm"] = [level + 0.5 for level in trace["amplitude_dbm"]]

