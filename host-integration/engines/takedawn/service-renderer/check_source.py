#!/usr/bin/env python3
"""Maintainer-only source/preparation check against an explicitly selected app build."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, default=Path("/media/user/660p1/c-site1/GamePKG/app"))
    parser.add_argument("--app-lib", required=True, type=Path, help="libgamepkg-*.rlib from the current app build")
    parser.add_argument("--slang-root", type=Path, help="Also run all templates through signed confined preparation")
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    subprocess.run(["python3", str(here / "prepare_templates.py")], check=True)
    with tempfile.TemporaryDirectory(prefix="takedawn-source-policy-") as name:
        scratch = Path(name)
        subprocess.run(["rustc", "--edition=2021", "-D", "warnings", str(here / "source_check.rs"),
                        "--extern", f"gamepkg={args.app_lib}", "-L", f"dependency={args.app_lib.parent}",
                        "-o", str(scratch / "check")], check=True)
        command = [str(scratch / "check"), str(args.app), str(here), str(scratch)]
        environment = os.environ.copy()
        if args.slang_root:
            command.append(str(args.slang_root))
            environment["GAMEPKG_SLANG_ROOT"] = str(args.slang_root)
        subprocess.run(command, env=environment, check=True)


if __name__ == "__main__":
    main()
