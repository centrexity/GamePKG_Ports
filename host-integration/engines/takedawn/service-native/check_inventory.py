#!/usr/bin/env python3
"""Maintainer whole-kit lowering; never a player requirement or admission exemption."""
import argparse
import hashlib
import json
from pathlib import Path
import os
import subprocess
import tarfile
import tempfile

KIT = '6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4'

def digest(data):
    return hashlib.sha256(data).hexdigest()

def validate(report, here, output, expected):
    assert report['source_kit_sha256'] == KIT and report['production_ready'] is False
    for field, path in [
        ('converter_main_sha256', here/'maintainer/src/main.rs'),
        ('converter_batch_sha256', here/'maintainer/src/batch.rs'),
        ('converter_lock_sha256', here/'maintainer/Cargo.lock'),
        ('inventory_sha256', here/'maintainer/reviewed_units.tsv'),
    ]:
        assert report[field] == digest(path.read_bytes()), field
    assert report['units'] == expected
    closed, rejected = report['closed_graph'], report['rejected']
    assert not set(closed) & set(rejected)
    assert len(closed) == report['closed_definitions']
    assert len(closed) + len(rejected) == report['unique_definitions']
    assert not set(closed) & set(report['duplicates'])
    for name, function in closed.items():
        assert name.startswith('sub_') and len(name) == 12
        int(name[4:], 16)
        assert function['unit'] in expected
        assert all(dependency in closed for dependency in function['dependencies'])
    for name, expected_hash in report['emitted_files'].items():
        parts = Path(name).parts
        assert not Path(name).is_absolute() and '..' not in parts and name.endswith('.rs')
        assert digest((output/name).read_bytes()) == expected_hash
    assert set(report['emitted_files']) == {str(p.relative_to(output)) for p in output.rglob('*.rs')}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True, help='new directory for unadmitted candidate source and graph')
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    output = args.output.resolve()
    if output.exists():
        raise RuntimeError('Refusing to overwrite an existing output')
    expected = dict(line.split('\t') for line in (here/'maintainer/reviewed_units.tsv').read_text().splitlines())
    assert len(expected) == 485
    kit = here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
    assert digest(kit.read_bytes()) == KIT
    target = args.target_dir.resolve()
    env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    manifest = here/'maintainer/Cargo.toml'
    subprocess.run(['cargo', 'test', '--offline', '--locked', '--manifest-path', str(manifest)], env=env, check=True)
    subprocess.run(['cargo', 'build', '--release', '--offline', '--locked', '--manifest-path', str(manifest)], env=env, check=True)
    tool = target/'release/takedawn-safe-source-preparation'
    with tempfile.TemporaryDirectory(prefix='gamepkg-reviewed-kit-') as temporary:
        inputs = Path(temporary)
        # Read only the pinned generated Rust members, never extract links,
        # paths, media, or arbitrary archive entries.
        with tarfile.open(kit) as archive:
            for name, expected_hash in expected.items():
                member = archive.getmember('prepared/src/'+name)
                assert member.isfile() and member.size <= 16*1024*1024
                data = archive.extractfile(member).read()
                assert digest(data) == expected_hash
                (inputs/name).write_bytes(data)
        negative = inputs/'must-not-exist'
        first = inputs/next(iter(expected))
        original = first.read_bytes()
        first.write_bytes(original+b'\n')
        denied = subprocess.run([str(tool), '--prepare-closed', str(inputs), str(negative)], capture_output=True)
        assert denied.returncode != 0 and not negative.exists()
        first.write_bytes(original)
        subprocess.run([str(tool), '--prepare-closed', str(inputs), str(output)], check=True)
    report = json.loads((output/'inventory.json').read_text())
    validate(report, here, output, expected)
    print(json.dumps({name: report[name] for name in ['definitions', 'unique_definitions', 'locally_convertible_definitions', 'closed_definitions']}, sort_keys=True))
    print('Verified inventory and candidate hashes only. Game source admission, separate object audits and semantic qualification remain required.')

if __name__ == '__main__':
    main()
