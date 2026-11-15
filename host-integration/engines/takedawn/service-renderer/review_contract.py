#!/usr/bin/env python3
"""Maintainer-only source inventory, not game admission or runtime telemetry."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tarfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
ARCHIVE = ROOT / "takedawn/assets/takedawn-source-kit.tar.gz"
INVENTORY = HERE / "source_inventory.json"
SOURCES = [
    "toolkit/src/d3d/d3d8_vsh_parse.h",
    "toolkit/src/d3d/d3d8_vsh_parse.c",
    "toolkit/src/d3d/d3d8_vsh_glsl.c",
    "toolkit/src/d3d/d3d8_combiners_parse.h",
    "toolkit/src/d3d/d3d8_combiners_parse.c",
    "toolkit/src/d3d/d3d8_combiners_glsl.c",
    "toolkit/src/d3d/d3d8_glsl.h",
    "toolkit/src/d3d/nv2a_pb_vulkan.c",
    "toolkit/src/d3d/nv2a_vk_core.c",
    "toolkit/src/kernel/nv2a_vsh_fields.h",
    "toolkit/src/kernel/nv2a_backend_common.h",
    "toolkit/src/kernel/nv2a_backend_common.c",
    "toolkit/src/kernel/nv2a_combiner.c",
    "toolkit/src/video/fb_present_vulkan.c",
    "toolkit/src/video/smaa.h",
    "toolkit/src/video/smaa.c",
    "toolkit/src/video/post_shaders.h",
]
LOCAL = [
    "media-bridge/native_d3d11.c",
    "media-bridge/native_cubemap.h",
    "media-bridge/private_edge_mask.h",
    "media-bridge/adapt_vertex_perspective.cmake",
    "media-bridge/presentation_policy.cmake",
    "media-bridge/attach.cmake",
    "media-bridge/windows_attach.cmake",
]
ENUMS = {
    "toolkit/src/d3d/d3d8_vsh_parse.h": [
        "NV2AVshMacOp", "NV2AVshIluOp", "NV2AVshRegType", "NV2AVshOutputReg",
    ],
    "toolkit/src/d3d/d3d8_combiners_parse.h": [
        "NV2ACombinerRegister", "NV2AInputMapping", "NV2AOutputMapping", "NV2ATextureMode",
    ],
    "toolkit/src/kernel/nv2a_backend_common.h": [
        "nv2a_topology", "nv2a_stencil_op", "nv2a_blend",
    ],
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def enum_values(source, name):
    source = re.sub(r"/\*.*?\*/|//[^\n]*", "", source, flags=re.S)
    match = re.search(r"\benum\s+" + re.escape(name) + r"\s*\{([^}]+)\}", source)
    if not match:
        raise ValueError("Missing reviewed enum: " + name)
    result, value = {}, -1
    for field in match.group(1).split(","):
        if not field.strip():
            continue
        token = re.fullmatch(r"\s*(\w+)\s*(?:=\s*(0[xX][0-9a-fA-F]+|\d+))?\s*", field)
        if not token:
            raise ValueError("Enum needs manual review: " + name)
        value = int(token[2], 0) if token[2] else value + 1
        result[token[1]] = value
    return result


def inventory():
    files, enums = {}, {}
    with tarfile.open(ARCHIVE, "r:gz") as archive:
        manifest = json.load(archive.extractfile("kit.json"))
        for name in SOURCES:
            member = archive.getmember(name)
            if not member.isfile() or member.size > 2 * 1024 * 1024:
                raise ValueError("Not a bounded source file: " + name)
            data = archive.extractfile(member).read()
            sha = digest(data)
            entry = manifest["files"][name]
            if sha != entry["sha256"] or len(data) != entry["bytes"]:
                raise ValueError("Source differs from kit inventory: " + name)
            files[name] = sha
            for enum in ENUMS.get(name, []):
                enums[enum] = enum_values(data.decode("utf-8"), enum)
    local = {name: digest((HERE.parent / name).read_bytes()) for name in LOCAL}
    return {
        "scope": "Static translator declarations only; no observed gameplay program/state coverage",
        "archive_sha256": digest(ARCHIVE.read_bytes()),
        "archive_sources_sha256": files,
        "ports_sources_sha256": local,
        "declared_enums": enums,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="Regenerate for deliberate source review")
    args = parser.parse_args()
    current = inventory()
    if args.write:
        INVENTORY.write_text(json.dumps(current, indent=2, sort_keys=True) + "\n")
        print("Wrote static renderer source inventory; gameplay coverage remains unqualified.")
    else:
        if json.loads(INVENTORY.read_text()) != current:
            raise SystemExit("FAIL: renderer inputs changed; review before regenerating inventory")
        print("PASS: 17 archive sources, 7 port sources and 11 enum families match reviewed inventory.")


if __name__ == "__main__":
    main()
