"""Download and verify a published Windows release without GitHub credentials."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request
import zipfile


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "RFWorkbench-Public-Check"})
    return urllib.request.urlopen(request, timeout=60)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", default="latest")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    base = "https://api.github.com/repos/Citroz31/rf-workbench"
    with fetch(base) as response:
        assert not json.load(response)["private"], "Repository must be public"
    endpoint = "/releases/latest" if args.tag == "latest" else "/releases/tags/" + args.tag
    with fetch(base + endpoint) as response:
        release = json.load(response)
    assert not release["draft"], "Release must be published"
    assets = {asset["name"]: asset for asset in release["assets"]}
    names = ["rf-workbench.exe", "RF-Workbench-Windows-x64-portable.zip", "SHA256SUMS.txt"]
    args.output.mkdir(parents=True, exist_ok=True)
    report = {"release": release["html_url"], "authentication": "none", "files": []}
    for name in names:
        asset = assets[name]
        assert asset["state"] == "uploaded"
        path = args.output / name
        digest = hashlib.sha256()
        with fetch(asset["browser_download_url"]) as response, path.open("wb") as target:
            while chunk := response.read(1024 * 1024):
                digest.update(chunk)
                target.write(chunk)
        assert path.stat().st_size == asset["size"], "Download size mismatch"
        assert "sha256:" + digest.hexdigest() == asset["digest"], "Download hash mismatch"
        report["files"].append({"name": name, "bytes": path.stat().st_size,
                                "sha256": digest.hexdigest(), "url": asset["browser_download_url"]})
    executable = (args.output / names[0]).read_bytes()
    assert executable[:2] == b"MZ", "Executable missing PE signature"
    with zipfile.ZipFile(args.output / names[1]) as archive:
        assert archive.testzip() is None
        assert archive.read("rf-workbench.exe") == executable, "ZIP executable differs"
        assert not any(n.endswith((".cmd", "python-path.txt")) for n in archive.namelist())
    for line in (args.output / names[2]).read_text(encoding="utf-8-sig").splitlines():
        expected, name = line.split(maxsplit=1)
        path = args.output / name.strip().lstrip("*")
        assert path.is_file(), "Checksum references a missing download"
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected
    (args.output / "verification.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
