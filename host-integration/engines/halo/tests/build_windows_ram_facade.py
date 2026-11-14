#!/usr/bin/env python3
"""Build an asset-free Windows file-API fixture with an existing cross-build SDK.

Maintainer Linux tool. Uses the configured native game's compiler flags and
actual file, handle, thread and disc implementations. It does not run the EXE.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True, help='new evidence directory')
args = parser.parse_args()
assert os.name == 'posix', 'This command reads the Linux cross-build recipe'
source = args.source.resolve()
work = args.output.resolve()
work.mkdir(parents=True, exist_ok=False)
root = Path(__file__).resolve().parents[1]
header = root / 'cache/gamepkg_ram_cache.h'
assert (source / 'port/linux/src/gamepkg_ram_cache.h').read_bytes() == header.read_bytes()
recipe = subprocess.check_output(['ninja', '-t', 'commands',
                                 'build/windows/obj/port/linux/src/xbox_files.o'],
                                cwd=source, text=True).splitlines()[-1]
words = shlex.split(recipe)
assert words.pop(0) == 'env', 'Expected the reviewed cross-toolchain environment'
env = dict(os.environ)
while words and '=' in words[0] and not words[0].startswith('-'):
    key, value = words.pop(0).split('=', 1)
    assert key in ['INCLUDE', 'LIB', 'PATH'], key
    env[key] = value
assert words and Path(words[0]).name.startswith('clang'), 'Expected Clang cross compiler'
base = []
index = 0
while index < len(words):
    word = words[index]
    if word in ['-MF', '-c', '-o']:
        index += 2
        continue
    if word == '-MMD':
        index += 1
        continue
    assert word not in [';', '&&', '|'], 'Shell commands are not compiler arguments'
    base.append(word)
    index += 1
fixture = work / 'fixture.c'
fixture.write_bytes((root / 'tests/windows_ram_facade.c').read_bytes())
inputs = [('files', source / 'port/linux/src/xbox_files.c'),
          ('kernel', source / 'port/linux/src/xbox_kernel.c'), ('fixture', fixture)]
objects = []
hashes = {}
binary = work / 'facade-fixture.exe'
with (work / 'build.log').open('wb') as log:
    for name, path in inputs:
        output = work / (name + '.o')
        command = [*base, '-ffunction-sections', '-fdata-sections', '-c', str(path), '-o', str(output)]
        result = subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
        assert result.returncode == 0, 'Compile failed; see ' + str(work / 'build.log')
        objects.append(str(output))
        hashes[str(path)] = hashlib.sha256(path.read_bytes()).hexdigest()
    for name in ['port/linux/src/xiso', 'port/windows/src/win32_files', 'port/windows/src/win32_posix']:
        path = source / ('build/windows/obj/' + name + '.o')
        objects.append(str(path))
        hashes[str(path)] = hashlib.sha256(path.read_bytes()).hexdigest()
    command = [base[0], '--target=i686-pc-windows-msvc', '-fuse-ld=lld', *objects,
               '-lkernel32', '-lwinmm', '-o', str(binary), '-Xlinker', '/subsystem:console',
               '-Xlinker', '/opt:ref', '-Xlinker', '/ignore:4099']
    result = subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
    assert result.returncode == 0, 'Link failed; see ' + str(work / 'build.log')
record = {'built': True, 'executed': False, 'binary': str(binary),
          'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'inputs': hashes,
          'header_sha256': hashlib.sha256(header.read_bytes()).hexdigest(),
          'scope': 'Actual Windows game file/handle code and thread/disc/filesystem objects; synthetic data only. Building does not establish execution or gameplay.'}
(work / 'artifact.json').write_text(json.dumps(record, indent=2) + '\n')
print('BUILT Windows file-API fixture; execute separately on Windows:', binary, flush=True)
