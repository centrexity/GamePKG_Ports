#!/usr/bin/env python3
"""Build and audit the typed ROM handle fixture, then run confined Linux children."""
import argparse
import hashlib
import importlib.util
import json
import subprocess
import tempfile
from pathlib import Path


def helper(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--app-lib", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    app = args.app.resolve(strict=True)
    library = args.app_lib.resolve(strict=True)
    here = Path(__file__).resolve().parent
    depot = helper("dependencies", here.parent / "service-save/check.py")
    disc = helper("disc", here.parent / "service-disc/check.py")
    sdk = subprocess.check_output([
        "python3", str(app / "tools/check_native_sdk_identity.py"),
        "--app-lib", str(library),
    ], text=True).strip()
    evidence = args.evidence.resolve()
    evidence.mkdir(exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="gamepkg-rom-handles-") as temp:
        root = Path(temp)
        image = root / "synthetic.iso"
        disc.fixture(image, large=True)
        exe = root / "admission"
        command = [
            "rustc", "--edition=2021", "-D", "warnings", str(here / "admission.rs"),
            "--extern", "gamepkg=" + str(library), "-L", "dependency=" + str(library.parent),
            "-o", str(exe),
        ]
        for name in ["tempfile", "toml", "serde_json"]:
            command += ["--extern", name + "=" + str(depot.dependency(library, name))]
        subprocess.run(command, check=True)
        subprocess.run([str(exe), str(app), str(here), str(evidence), str(image)], check=True)
    receipts = {}
    for path in evidence.glob("receipt-*/services.json"):
        raw = path.read_bytes()
        receipt = json.loads(raw)
        assert receipt["game"]["sdk"] == sdk and receipt["plan"]["optimize"]
        assert "lto=off" in receipt["arguments"]
        assert len(receipt["raw_objects"]) == len(receipt["game"]["objects"]) == 8
        receipts[path.parent.name] = hashlib.sha256(raw).hexdigest()
    assert len(receipts) == 4
    (evidence / "qualification.json").write_text(json.dumps({
        "schema": 1, "native_sdk": sdk, "receipts": receipts,
        "execution": "confined Linux32/64, Windows32/64 source/object/link only",
        "scope": "synthetic read-only ROM handles, no title entry",
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
