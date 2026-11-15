#!/usr/bin/env python3
"""Maintainer smoke check against an explicitly selected current GamePKG CLI.

Uses an isolated library and temporary test creator; never releases packages or
changes player data. Headless frames do not qualify visible gameplay or audio.
"""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    args = parser.parse_args()
    cli = args.cli.resolve(strict=True)
    root = Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix="gamepkg-current-ports-") as name:
        scratch = Path(name)
        config = scratch / "settings.toml"
        config.write_text("automatic_update_checks=false\n")
        env = dict(os.environ, GAMEPKG_HOME=str(scratch / "library"),
                   GAMEPKG_CONFIG=str(config))

        def run(*arguments):
            result = subprocess.run([str(cli), *map(str, arguments)], env=env,
                                    capture_output=True, text=True, timeout=600)
            if result.returncode:
                raise RuntimeError(f"{arguments[0]} failed: {result.stdout}{result.stderr}")
            return result.stdout

        key = scratch / "temporary-test-creator.key"
        run("keygen", "-o", key)
        for port in ["2048", "arkanoo"]:
            archive = scratch / f"{port}.gamepkg"
            run("pack", root / port, "--key", key, "-o", archive)
            run("verify", archive)
            run("install", archive, "--trust-creator")
            game = "com.gamepkg." + port
            run("build", game, "--target", "native")
            output = run("run", game, "--target", "native", "--frames", "120")
            if "Ran 120" not in output:
                raise RuntimeError(f"{port}: no completed-frame report: {output}")
            print(f"PASS {port}: signed, verified, installed, native-built, 120 headless frames", flush=True)
    print("No visible gameplay, physical controls, audible audio or frame-pacing qualification.")


if __name__ == "__main__":
    main()
