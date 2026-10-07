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


def require(condition, message):
    if not condition:
        raise ValueError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", default="latest")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    base = "https://api.github.com/repos/Citroz31/rf-workbench"
    with fetch(base) as response:
        require(not json.load(response)["private"], "Repository must be public")
    endpoint = "/releases/latest" if args.tag == "latest" else "/releases/tags/" + args.tag
    with fetch(base + endpoint) as response:
        release = json.load(response)
    require(not release["draft"], "Release must be published")
    require(args.tag == "latest" or release["tag_name"] == args.tag, "Unexpected release tag")
    assets = {asset["name"]: asset for asset in release["assets"]}
    names = ["rf-workbench.exe", "RF-Workbench-Windows-x64-portable.zip", "SHA256SUMS.txt"]
    args.output.mkdir(parents=True, exist_ok=True)
    report = {"release": release["html_url"], "authentication": "none", "files": []}
    for name in names:
        asset = assets[name]
        require(asset["state"] == "uploaded", "Asset must be fully uploaded")
        path = args.output / name
        digest = hashlib.sha256()
        with fetch(asset["browser_download_url"]) as response, path.open("wb") as target:
            while chunk := response.read(1024 * 1024):
                digest.update(chunk)
                target.write(chunk)
        require(path.stat().st_size == asset["size"], "Download size mismatch")
        require("sha256:" + digest.hexdigest() == asset["digest"], "Download hash mismatch")
        report["files"].append({"name": name, "bytes": path.stat().st_size,
                                "sha256": digest.hexdigest(), "url": asset["browser_download_url"]})
    executable = (args.output / names[0]).read_bytes()
    require(executable[:2] == b"MZ", "Executable missing PE signature")
    with zipfile.ZipFile(args.output / names[1]) as archive:
        require(archive.testzip() is None, "ZIP CRC check failed")
        require(archive.read("rf-workbench.exe") == executable, "ZIP executable differs")
        require(not any(n.endswith((".cmd", "python-path.txt")) for n in archive.namelist()),
                "ZIP contains a launcher or a machine-specific Python path")
    seen = set()
    for line in (args.output / names[2]).read_text(encoding="utf-8-sig").splitlines():
        expected, name = line.split(maxsplit=1)
        name = name.strip().lstrip("*")
        require(name in names[:2] and name not in seen, "Unexpected or duplicate checksum entry")
        seen.add(name)
        path = args.output / name
        require(path.is_file(), "Checksum references a missing download")
        require(hashlib.sha256(path.read_bytes()).hexdigest() == expected, "Checksum file mismatch")
    require(seen == set(names[:2]), "Checksum file must cover EXE and ZIP")
    (args.output / "verification.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
