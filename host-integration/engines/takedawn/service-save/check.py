#!/usr/bin/env python3
"""Maintainer checks using the actual app session and unmodified generated client."""
import argparse
import json
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import tarfile
import re


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
        archive_path = here.parents[3] / "takedawn/assets/takedawn-source-kit.tar.gz"
        if hashlib.sha256(archive_path.read_bytes()).hexdigest() != "6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4":
            raise RuntimeError("Changed source kit requires saveguard review")
        with tarfile.open(archive_path, "r:gz") as archive:
            source = archive.extractfile("toolkit/src/kernel/kernel_saveguard.c").read().decode()
        functions = []
        for function in ["pend_of", "problem_set", "pend_drop", "pend_add", "pend_retry"]:
            start = re.search(r"^static [^\n]*\b" + function + r"\(", source, re.MULTILINE).start()
            opening = source.index("{", start)
            depth = 1
            end = opening + 1
            # Exact pinned functions contain no braces inside strings/comments.
            # The whole-kit hash above prevents silently changing this premise.
            while depth:
                depth += (source[end] == "{") - (source[end] == "}")
                end += 1
            functions.append(source[start:end])
        (scratch / "pending.inc").write_text("\n\n".join(functions) + "\n")
        subprocess.run(["cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                        "-I", str(scratch), str(here / "pending_oracle.c"),
                        "-o", str(scratch / "pending-oracle")], check=True)
        subprocess.run(["rustc", "--edition=2021", "-D", "warnings", "--test", str(here / "state_tests.rs"),
                        "-o", str(scratch / "state-tests")], check=True)
        subprocess.run([str(scratch / "state-tests")], env=dict(os.environ,
                       TAKEDAWN_SAVE_PENDING_ORACLE=str(scratch / "pending-oracle")), check=True)
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
