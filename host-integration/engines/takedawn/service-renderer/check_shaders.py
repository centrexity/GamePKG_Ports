#!/usr/bin/env python3
"""Maintainer preparation checks; no game admission, signing or player dependency."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile
import tarfile


def run(args, **kwargs):
    subprocess.run([str(a) for a in args], check=True, **kwargs)


def library(deps, name):
    choices = list(deps.glob(f"lib{name}-*.rlib"))
    if not choices:
        raise SystemExit(f"Build the app's maintainer dependencies first: missing {name}")
    return max(choices, key=lambda p: p.stat().st_mtime)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--slang-root", type=Path, default=os.environ.get("GAMEPKG_SLANG_ROOT"))
    parser.add_argument("--deps", required=True, type=Path, help="Existing app Cargo target/debug/deps directory")
    parser.add_argument("--app", type=Path, default=Path("/media/user/660p1/c-site1/GamePKG/app"))
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--hardware", action="store_true", help="Require hardware Vulkan for execution vectors")
    mode.add_argument("--software-diagnostic", action="store_true", help="Separate nonqualifying software execution diagnostic")
    args = parser.parse_args()
    if args.slang_root is None:
        raise SystemExit("Set --slang-root or GAMEPKG_SLANG_ROOT to pinned Slang 2026.19")
    compiler = args.slang_root / "bin/slangc"
    version = subprocess.check_output([str(compiler), "-version"], stderr=subprocess.STDOUT).decode()
    if "2026.19" not in version:
        raise SystemExit("Requires pinned Slang 2026.19")
    here = Path(__file__).resolve().parent
    run(["python3", here / "prepare_templates.py"])
    with tempfile.TemporaryDirectory(prefix="takedawn-shaders-") as name:
        stage = Path(name)
        outputs = []
        for mask in range(16):
            for entry, shader_stage in [("vertexMain", "vertex"), ("fragmentMain", "fragment")]:
                for backend in ["wgsl", "spirv"]:
                    output = stage / f"{mask:02}-{shader_stage}.{'spv' if backend == 'spirv' else 'wgsl'}"
                    command = [compiler, here / f"shaders/template_{mask:02}.slang", "-entry", entry,
                               "-stage", shader_stage, "-target", backend, "-o", output]
                    if backend == "spirv":
                        command += ["-profile", "glsl_450", "-DGAMEPKG_SEPARATE_SAMPLER=1"]
                    run(command)
                    outputs.append(output)
        run(["rustc", "--edition=2021", "-D", "warnings", here / "shader_check.rs",
             "-L", f"dependency={args.deps}", "--extern", f"naga={library(args.deps, 'naga')}",
             "-o", stage / "check"])
        run([stage / "check", *outputs])
        if args.hardware or args.software_diagnostic:
            run(["python3", here / "review_contract.py"])
            with tarfile.open(here.parents[3] / "takedawn/assets/takedawn-source-kit.tar.gz", "r:gz") as archive:
                for source in ["d3d/d3d8_vsh_parse.c", "d3d/d3d8_vsh_parse.h", "d3d/d3d8_combiners_parse.c",
                               "d3d/d3d8_combiners_parse.h", "kernel/nv2a_vsh_fields.h", "kernel/nv2a_combiner.h", "kernel/nv2a_combiner.c"]:
                    output = stage / source
                    output.parent.mkdir(parents=True, exist_ok=True)
                    output.write_bytes(archive.extractfile("toolkit/src/" + source).read())
            run(["cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-I" + str(stage / "d3d"), "-I" + str(stage / "kernel"),
                 here / "oracle.c", stage / "d3d/d3d8_vsh_parse.c", stage / "d3d/d3d8_combiners_parse.c",
                 stage / "kernel/nv2a_combiner.c", "-lm", "-o", stage / "oracle"])
            run(["rustc", "--edition=2021", "-D", "warnings", "--crate-name", "takedawn_nvi1", "--crate-type", "rlib",
                 here / "nvi1.rs", "-o", stage / "libtakedawn_nvi1.rlib"])
            run(["rustc", "--edition=2021", "-D", "warnings", "--crate-name", "host_nvi1", "--crate-type", "rlib",
                 args.app / "src/shaders/nv2a.rs", "-o", stage / "libhost_nvi1.rlib"])
            command = ["rustc", "--edition=2021", "-D", "warnings", here / "gpu_vectors.rs", "-L", f"dependency={args.deps}"]
            for lib in ["wgpu", "pollster"]:
                command += ["--extern", f"{lib}={library(args.deps, lib)}"]
            for lib in ["takedawn_nvi1", "host_nvi1"]:
                command += ["--extern", f"{lib}={stage / ('lib' + lib + '.rlib')}"]
            run([*command, "-o", stage / "vectors"])
            diagnostic = ["--software-diagnostic"] if args.software_diagnostic else []
            environment = os.environ.copy()
            environment["TAKEDAWN_NVI1_ORACLE"] = str(stage / "oracle")
            run([stage / "vectors", *diagnostic, stage / "00-vertex.wgsl", stage / "00-fragment.wgsl"], env=environment)
        else:
            print("Compilation/structure only; no GPU execution qualification requested.")


if __name__ == "__main__":
    main()
