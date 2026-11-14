#!/usr/bin/env python3
"""Asset-free process-memory cache checks; maintainer Python only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', required=True, type=Path, help='new evidence directory')
parser.add_argument('--source', type=Path, help='patched engine, required for Windows thread helpers')
parser.add_argument('--cc', default='clang')
parser.add_argument('--cflag', action='append', default=[])
parser.add_argument('--sanitize', action='store_true')
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
work = args.output.resolve()
work.mkdir(parents=True, exist_ok=False)
header = (root / 'cache/gamepkg_ram_cache.h').read_text()
fixture = (root / 'tests/ram_cache.c').read_text()
if args.source:
    assert (args.source / 'port/linux/src/gamepkg_ram_cache.h').read_text() == header
windows = os.name == 'nt'
assert not windows or args.source, 'Windows requires the reviewed native thread helpers'
assert not windows or not args.sanitize, 'Use Linux for the memory-instrumented run'
variants = [('actual', header, None)]
if not windows:
    tail = '''        if (size % GP_RAM_PAGE && node->pages[size / GP_RAM_PAGE])
            memset(node->pages[size / GP_RAM_PAGE] + size % GP_RAM_PAGE, 0,
                GP_RAM_PAGE - (size_t)(size % GP_RAM_PAGE));'''
    readonly = '    if (node->read_only) { errno = EACCES; goto failed; }'
    quota = 'size > gp_ram_capacity - (gp_ram_logical - node->size)'
    for old in [tail, readonly, quota]:
        assert header.count(old) == 1
    variants += [
        ('retained-truncated-tail', header.replace(tail, ''), 'bytes[i] == 0'),
        ('ignored-readonly', header.replace(readonly, ''), 'gp_ram_write(a, data, 1, 1, 0)'),
        ('ignored-logical-budget', header.replace(quota, '0'), 'gp_ram_truncate(b) == -1'),
    ]
record = {'scope': 'Native RAM cache core, real Windows thread helpers when applicable; not full game filesystem or gameplay qualification.',
          'header_sha256': hashlib.sha256(header.encode()).hexdigest(), 'checks': [], 'passed': False}
try:
    for name, source, expected in variants:
        directory = work / name
        directory.mkdir()
        (directory / 'gamepkg_ram_cache.h').write_text(source)
        (directory / 'fixture.c').write_text(fixture)
        binary = directory / ('fixture.exe' if windows else 'fixture')
        command = [args.cc, '-std=gnu89', '-g', '-O1', '-ffp-contract=off', '-I', str(directory)]
        if windows:
            command += ['--target=i686-pc-windows-msvc', '-fuse-ld=lld', '-msse2',
                        '-D_USE_32BIT_TIME_T', '-D_CRT_SECURE_NO_WARNINGS', '-D_CRT_NONSTDC_NO_WARNINGS',
                        '-I', str(args.source / 'port/windows/include/posix'),
                        '-I', str(args.source / 'port/linux/src')]
        else:
            command += ['-pthread']
        if args.sanitize:
            command += ['-fsanitize=address,undefined']
        command += args.cflag + [str(directory / 'fixture.c')]
        if windows:
            command += [str(args.source / 'port/windows/src/win32_posix.c'), '-lkernel32', '-lwinmm']
        command += ['-o', str(binary)]
        build = subprocess.run(command, capture_output=True, text=True)
        (directory / 'build.log').write_text(build.stdout + build.stderr)
        assert build.returncode == 0, 'See ' + str(directory / 'build.log')
        env = dict(os.environ, ASAN_OPTIONS='detect_leaks=1:abort_on_error=1', UBSAN_OPTIONS='halt_on_error=1')
        run = subprocess.run([str(binary)], capture_output=True, text=True, env=env, timeout=30)
        (directory / 'run.log').write_text(run.stdout + run.stderr)
        if expected:
            assert run.returncode != 0 and expected in run.stderr, (name, run.stdout, run.stderr)
            # A sanitizer error is not the expected deliberate assertion.
            assert 'ERROR: AddressSanitizer' not in run.stderr and 'runtime error:' not in run.stderr
        else:
            assert run.returncode == 0 and 'PASS RAM cache' in run.stdout, (run.stdout, run.stderr)
            layout = subprocess.run([str(binary), '--layout'], capture_output=True, text=True, env=env, timeout=30)
            (directory / 'layout.log').write_text(layout.stdout + layout.stderr)
            assert layout.returncode == 0 and 'PASS RAM cache full six-map layout' in layout.stdout, (layout.stdout, layout.stderr)
        record['checks'].append({'name': name, 'exit': run.returncode,
                                 'expected_rejection': expected is not None,
                                 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest()})
        print('PASS', name, flush=True)
    record['passed'] = True
finally:
    (work / 'result.json').write_text(json.dumps(record, indent=2) + '\n')
