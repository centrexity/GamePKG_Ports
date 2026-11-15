#!/usr/bin/env python3
"""Maintainer-only expansion into fixed self-contained sources BEFORE signing."""
import argparse
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="Refresh reviewed prepared source files")
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    vertex = (here / "template-source/nvi1.slang").read_text()
    fragment = (here / "template-source/nvi1_fragment.slang").read_text()
    marker = '#include "nvi1_fragment.slang"'
    if vertex.count(marker) != 1:
        raise SystemExit("Changed authoring boundary needs review")
    source = vertex.replace(marker, fragment)
    for mask in range(16):
        prepared = f"// Prepared by maintainer before signing; never expanded by the game.\n#define NVI1_CUBE_MASK {mask}\n" + source
        path = here / f"shaders/template_{mask:02}.slang"
        if args.write:
            path.write_text(prepared)
        elif path.read_text() != prepared:
            raise SystemExit(f"Prepared source differs: {path.name}")
    print("PASS: 16 fixed self-contained candidate templates match authoring sources")


if __name__ == "__main__":
    main()
