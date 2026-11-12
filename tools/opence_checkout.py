#!/usr/bin/env python3
"""Pin and check an OpenCE checkout without packaging Halo game data.

Players do not need this. It clones the commit in halo/UPSTREAM.json, rejects
disc images and map caches, and can run OpenCE's own Linux build in that
checkout. The resulting binary is not a GamePKG engine.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

PORTS = Path(__file__).resolve().parents[1]
PIN_PATH = PORTS / "tools" / "opence-pin.json"
# Commercial game-data names. OpenCE's source tree has none of these.
FORBIDDEN_SUFFIXES = {".iso", ".xiso", ".map", ".wad", ".sfc", ".smc", ".rom"}
DATA_DIRECTORIES = {"maps", "custom_maps"}


class CheckoutError(Exception):
    pass


def load_pin() -> dict:
    pin = json.loads(PIN_PATH.read_text())
    commit = pin["commit"]
    if len(commit) != 40 or any(c not in "0123456789abcdef" for c in commit):
        raise CheckoutError("UPSTREAM.json commit must be 40 lowercase hex characters")
    if pin.get("vendored") is not False:
        raise CheckoutError("OpenCE must stay unvendored")
    if pin.get("license") != "AGPL-3.0-or-later":
        raise CheckoutError("pinned OpenCE license must stay AGPL-3.0-or-later")
    digest = pin["license_sha256"]
    if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
        raise CheckoutError("UPSTREAM.json license_sha256 is not 64 lowercase hex characters")
    return pin


def repository_roots() -> list[Path]:
    roots = [PORTS]
    parent = PORTS.parent
    if (parent / ".gitmodules").is_file() or (parent / "Cargo.toml").is_file():
        roots.append(parent)
    return roots


def assert_outside_repository(path: Path) -> Path:
    resolved = path.expanduser().resolve()
    for root in repository_roots():
        root = root.resolve()
        if resolved == root or root in resolved.parents:
            raise CheckoutError(f"{resolved} is inside {root}; choose a directory outside the repository")
    return resolved


def forbidden_game_data(root: Path) -> list[str]:
    found: list[str] = []
    if not root.is_dir():
        raise CheckoutError(f"{root} is not a directory")
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
        current = Path(dirpath)
        kept: list[str] = []
        for name in dirnames:
            if name == ".git":
                continue
            if name.casefold() in DATA_DIRECTORIES:
                found.append((current / name).relative_to(root).as_posix() + "/")
                if (current / name).is_symlink():
                    continue
            kept.append(name)
        dirnames[:] = kept
        for name in filenames:
            path = current / name
            relative = path.relative_to(root)
            parts = {part.casefold() for part in relative.parts}
            if path.suffix.casefold() in FORBIDDEN_SUFFIXES or parts & DATA_DIRECTORIES:
                found.append(relative.as_posix())
    return sorted(found)


def check_license(checkout: Path, pin: dict) -> None:
    license_path = checkout / "LICENSE"
    if not license_path.is_file():
        raise CheckoutError("checkout has no LICENSE")
    data = license_path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != pin["license_sha256"]:
        raise CheckoutError(f"LICENSE sha256 {digest} does not match the pin")
    if b"GNU AFFERO GENERAL PUBLIC LICENSE" not in data:
        raise CheckoutError("LICENSE is not the GNU Affero General Public License")


def git_head(checkout: Path) -> str:
    if not (checkout / ".git").exists():
        raise CheckoutError(f"{checkout} is not a git checkout")
    result = subprocess.run(
        ["git", "-C", str(checkout), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise CheckoutError(result.stderr.strip() or "git rev-parse failed")
    return result.stdout.strip()


def verify(checkout: Path) -> dict:
    pin = load_pin()
    checkout = checkout.expanduser().resolve()
    head = git_head(checkout)
    if head != pin["commit"]:
        raise CheckoutError(f"HEAD {head} is not pinned commit {pin['commit']}")
    for relative in ("configure.py", "source", "port/linux"):
        if not (checkout / relative).exists():
            raise CheckoutError(f"checkout is missing {relative}")
    check_license(checkout, pin)
    forbidden = forbidden_game_data(checkout)
    if forbidden:
        preview = ", ".join(forbidden[:8])
        raise CheckoutError(f"checkout contains game data that must not be packaged: {preview}")
    return pin


def fetch(dest: Path) -> Path:
    pin = load_pin()
    dest = assert_outside_repository(dest)
    if dest.exists():
        raise CheckoutError(f"{dest} already exists")
    clone = subprocess.run(
        ["git", "clone", "--no-tags", pin["clone_url"], str(dest)],
        check=False,
    )
    if clone.returncode != 0:
        raise CheckoutError(f"git clone failed with status {clone.returncode}")
    checkout = subprocess.run(
        ["git", "-C", str(dest), "checkout", "--detach", pin["commit"]],
        check=False,
    )
    if checkout.returncode != 0:
        raise CheckoutError(f"git checkout failed with status {checkout.returncode}")
    verify(dest)
    return dest


def build(checkout: Path, portable: bool) -> Path:
    pin = verify(checkout)
    if shutil_which("ninja") is None:
        raise CheckoutError("ninja is not installed")
    checkout = checkout.expanduser().resolve()
    command = [sys.executable, "configure.py", "--release", "--pgo=off", "--lto=off"]
    if portable:
        command.append("--portable")
    configured = subprocess.run(command, cwd=checkout, check=False)
    if configured.returncode != 0:
        raise CheckoutError(f"configure.py failed with status {configured.returncode}")
    compiled = subprocess.run(["ninja", "linux"], cwd=checkout, check=False)
    if compiled.returncode != 0:
        raise CheckoutError(f"ninja linux failed with status {compiled.returncode}")
    binary = checkout / "build" / "linux" / "halo"
    if not binary.is_file():
        raise CheckoutError("build finished without build/linux/halo")
    forbidden = forbidden_game_data(checkout)
    if forbidden:
        preview = ", ".join(forbidden[:8])
        raise CheckoutError(f"build tree contains game data that must stay out of packages: {preview}")
    print(f"Built upstream OpenCE {pin['commit']} at {binary}")
    print("This is not a GamePKG engine. Keep your disc image outside the source tree.")
    return binary


def shutil_which(name: str) -> str | None:
    import shutil
    return shutil.which(name)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    verify_cmd = sub.add_parser("verify", help="check a checkout against the pin")
    verify_cmd.add_argument("checkout", type=Path)
    fetch_cmd = sub.add_parser("fetch", help="clone the pinned commit outside this repository")
    fetch_cmd.add_argument("--dest", type=Path, required=True)
    build_cmd = sub.add_parser("build", help="configure and ninja linux in a verified checkout")
    build_cmd.add_argument("--checkout", type=Path, required=True)
    build_cmd.add_argument("--portable", action="store_true", help="build SDL3 and use the Debian 11 sysroot")
    args = parser.parse_args(argv)
    try:
        if args.command == "verify":
            pin = verify(args.checkout)
            print(f"OpenCE {pin['commit']} matches the pin and contains no packaged game data")
        elif args.command == "fetch":
            path = fetch(args.dest)
            print(f"Fetched pinned OpenCE to {path}")
        else:
            build(args.checkout, args.portable)
    except CheckoutError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
