"""Maintainer-only Halo game-port policy regression, without game media."""
import argparse
import hashlib
import json
from pathlib import Path
import resource
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cc', default='clang-18')
    parser.add_argument('--cflag', action='append', default=[])
    parser.add_argument('--sanitize', action='store_true')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    root = Path(__file__).resolve().parents[1]
    output = args.output.resolve()
    results = []
    for name in ['positive', 'compaction']:
        work = output / name
        (work / 'input').mkdir(parents=True, exist_ok=True)
        (work / 'tests').mkdir(exist_ok=True)
        code = (root / 'input/halo_gamepad_ports.h').read_text()
        if name == 'compaction':
            anchor = 'int i, j, count = 0, single = -1, present, assigned, target;'
            assert code.count(anchor) == 1
            code = code.replace(anchor, anchor + '\n    for (i = 0; i < 4; i++) state->devices[i] = 0;')
        (work / 'input/halo_gamepad_ports.h').write_text(code)
        shutil.copyfile(root / 'tests/gamepad_ports.c', work / 'tests/gamepad_ports.c')
        binary = work / 'check'
        flags = ['-fsanitize=address,undefined', '-fno-omit-frame-pointer'] if args.sanitize else []
        build = subprocess.run([args.cc, '-std=gnu89', '-O2', '-g', '-Wall', '-Wextra', '-Werror',
                                *args.cflag, *flags, str(work / 'tests/gamepad_ports.c'), '-o', str(binary)],
                               capture_output=True, text=True, timeout=60)
        (work / 'build.log').write_text(build.stdout + build.stderr)
        assert build.returncode == 0, build.stderr
        run = subprocess.run([str(binary)], capture_output=True, text=True, timeout=30)
        (work / 'run.log').write_text(run.stdout + run.stderr)
        if name == 'positive':
            assert run.returncode == 0 and '4096 stable hotplug topologies' in run.stdout, run.stderr
        else:
            assert run.returncode != 0 and 'Assertion' in run.stderr and \
                'ports[1] == 0 && ports[0] == 1' in run.stderr, run.stderr
        results.append({'name': name, 'exit': run.returncode, 'passed': True})
    receipt = {'results': results, 'sanitize': args.sanitize,
               'source_sha256': {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
                                 for path in [root / 'input/halo_gamepad_ports.h', root / 'tests/gamepad_ports.c']}}
    (output / 'result.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print('PASS Halo game-port policy and exact-assertion compaction negative control')


if __name__ == '__main__':
    main()
