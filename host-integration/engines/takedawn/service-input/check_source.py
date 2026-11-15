#!/usr/bin/env python3
"""Maintainer-only: admit the exact controller adapter using the current app build."""
import argparse
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--app-lib", type=Path, required=True,
                        help="libgamepkg-*.rlib from this app's current build")
    args = parser.parse_args()
    app = args.app.resolve(strict=True)
    library = args.app_lib.resolve(strict=True)
    here = Path(__file__).resolve().parent
    subprocess.run(["sh", str(here / "check.sh"), str(app)], check=True)
    with tempfile.TemporaryDirectory(prefix="takedawn-pad-admission-") as name:
        scratch = Path(name)
        executable = scratch / "check"
        subprocess.run([
            "rustc", "--edition=2021", "-D", "warnings", str(here / "source_check.rs"),
            "--extern", f"gamepkg={library}", "-L", f"dependency={library.parent}",
            "-o", str(executable),
        ], check=True)
        subprocess.run([str(executable), str(app), str(here), str(scratch)], check=True)


if __name__ == "__main__":
    main()
