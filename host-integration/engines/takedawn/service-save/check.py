#!/usr/bin/env python3
"""Maintainer checks using the actual app session and unmodified generated client."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def dependency(library, name):
    # Feature/profile variants coexist in deps. Picking the newest rlib can
    # accidentally link a different cap_std type from the one the host uses.
    fingerprints = library.parent.parent / ".fingerprint"
    app_hash = library.stem.rsplit("-", 1)[1]
    record = json.loads((fingerprints / f"gamepkg-{app_hash}" / "lib-gamepkg.json").read_text())
    expected = next(item[3] for item in record["deps"] if item[1] == name)
    candidates = []
    for fingerprint in fingerprints.glob(f"*/lib-{name}"):
        if int.from_bytes(bytes.fromhex(fingerprint.read_text()), "little") == expected:
            artifact_hash = fingerprint.parent.name.rsplit("-", 1)[1]
            artifact = library.parent / f"lib{name}-{artifact_hash}.rlib"
            if artifact.is_file():
                candidates.append(artifact)
    if len(candidates) != 1:
        raise RuntimeError(f"Cannot resolve exact {name} artifact for {library.name}")
    return candidates[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--app-lib", type=Path, required=True)
    args = parser.parse_args()
    app = args.app.resolve(strict=True)
    library = args.app_lib.resolve(strict=True)
    here = Path(__file__).resolve().parent
    deps = library.parent
    with tempfile.TemporaryDirectory(prefix="takedawn-save-service-") as name:
        scratch = Path(name)
        client = scratch / "libgamepkg_client.rlib"
        subprocess.run(["rustc", "--edition=2021", "-D", "warnings", "--crate-type", "rlib",
                        "--crate-name", "gamepkg", str(app / "engines/native-services/client.rs"),
                        "-o", str(client)], check=True)
        command = ["rustc", "--edition=2021", "-D", "warnings", "--test", str(here / "tests.rs"),
                   "--extern", f"gamepkg={client}", "--extern", f"host={library}",
                   "-L", f"dependency={deps}", "-o", str(scratch / "tests")]
        for name in ["cap_std", "tempfile"]:
            candidate = dependency(library, name)
            command += ["--extern", f"{name}={candidate}"]
        subprocess.run(command, check=True)
        subprocess.run([str(scratch / "tests"), "--test-threads=1"],
                       env=dict(os.environ, GAMEPKG_APP=str(app)), check=True)
        subprocess.run(["rustc", "--edition=2021", "-D", "warnings", str(here / "source_check.rs"),
                        "--extern", f"gamepkg={library}", "-L", f"dependency={deps}",
                        "-o", str(scratch / "source-check")], check=True)
        subprocess.run([str(scratch / "source-check"), str(app), str(here), str(scratch)], check=True)


if __name__ == "__main__":
    main()
