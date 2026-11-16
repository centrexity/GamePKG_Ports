#!/usr/bin/env python3
"""Maintainer original timer-table differential; C/Python are not player requirements."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app', type=Path, required=True)
    p.add_argument('--app-lib', type=Path)
    p.add_argument('--wine', action='store_true')
    p.add_argument('--evidence', type=Path)
    a = p.parse_args()
    app = a.app.resolve(strict=True)
    here = Path(__file__).resolve().parent
    pins = json.loads((here / 'reference_inputs.json').read_text())
    kit = here.parents[3] / 'takedawn/assets/takedawn-source-kit.tar.gz'
    assert digest(kit) == pins['source_kit_sha256']
    with tarfile.open(kit) as t:
        original = t.extractfile(pins['member']).read()
    assert hashlib.sha256(original).hexdigest() == pins['source_sha256']
    reference = (here / 'reference.c.in').read_text()
    clock_bodies = {}
    for name, pin in pins['functions'].items():
        body = original[pin['start']:pin['end']]
        assert hashlib.sha256(body).hexdigest() == pin['sha256']
        body = body.decode()
        if name in ['host_ms', 'guest_ms']:
            clock_bodies[name] = body
            continue
        if name == 'set':
            assert body.count('long period_ms') == 1
            body = body.replace('long period_ms', 'int32_t period_ms')
        if name == 'fire':
            assert body.count('        fired++;') == 1
            body = body.replace('        fired++;', '        fired++; reference_fired(fired_va, dpc);')
        reference = reference.replace('/* PINNED_' + name.upper() + ' */', body)
    copies = []
    for index in range(64):
        copies.append('#define kernel_host_ms clock_host_' + str(index) + '\n' +
                      '#define kernel_clock_ms clock_guest_' + str(index) + '\n' +
                      clock_bodies['host_ms'] + '\n' + clock_bodies['guest_ms'] + '\n' +
                      '#undef kernel_clock_ms\n#undef kernel_host_ms\n')
    reference = reference.replace('/* PINNED_CLOCKS */', '\n'.join(copies)).replace('/* CLOCK_CASES */',
        '\n'.join('case ' + str(index) + ':return clock_guest_' + str(index) + '();' for index in range(64)))
    assert '/* PINNED_' not in reference
    modules = [('client', app / 'engines/native-services/client.rs'),
               ('local', app / 'engines/native-services/local_bindings.rs')]
    for name, relative in [
        ('memory', '../service-native/memory.rs'),
        ('call_state', '../service-native/call_state.rs'),
        ('registers', '../service-native/registers.rs'),
        ('heap', '../service-heap/heap.rs'),
        ('contiguous', '../service-contiguous/contiguous.rs'),
        ('heap_service', '../service-heap/service.rs'),
        ('kernel_pool', '../service-kernel-pool/kernel_pool.rs'),
        ('wait_service', '../service-kernel-wait/wait.rs'),
        ('queue', '../service-kernel-dpc/queue.rs'),
        ('timers', 'timers.rs'), ('timer_clock', 'fixture_clock.rs'),
        ('production_clock', 'clock.rs'), ('cases', 'cases.rs'), ('controls', 'controls.rs'), ('probe', 'probe.rs'),
    ]:
        modules.append((name, here / relative))
    with tempfile.TemporaryDirectory(prefix='gamepkg-kernel-timers-') as temp:
        root = Path(temp)
        c = root / 'reference.c'
        c.write_text(reference)
        source = root / 'main.rs'
        source.write_text('#![allow(dead_code)]\n' + ''.join(
            '#[path=' + json.dumps(str(path)) + ']mod ' + name + ';\n'
            for name, path in modules) + (here / 'runtime.rs').read_text())
        for target, compiler, extra in [
            ('x86_64-unknown-linux-gnu', 'gcc', []),
            ('i686-unknown-linux-gnu', 'gcc-11', ['-m32']),
            ('x86_64-pc-windows-gnu', 'x86_64-w64-mingw32-gcc', []),
            ('i686-pc-windows-gnu', 'i686-w64-mingw32-gcc', []),
        ]:
            obj = root / (target + '.o')
            subprocess.run([compiler, *extra, '-std=c11', '-Wall', '-Wextra', '-Werror',
                            '-O2', '-c', str(c), '-o', str(obj)], check=True)
            exe = root / (target + ('.exe' if 'windows' in target else ''))
            cmd = ['rustc', '--edition=2021', '-D', 'warnings', '-O', '--target', target,
                   str(source), '-C', 'link-arg=' + str(obj), '-o', str(exe)]
            if target == 'i686-unknown-linux-gnu':
                cmd += ['-C', 'linker=gcc-11', '-C', 'link-arg=-lssp_nonshared']
            if 'windows' in target:
                # C oracle shell only; never linked into audited game objects.
                cmd += ['-C', 'link-arg=-lmingwex', '-C', 'link-arg=-lmsvcrt', '-C', 'link-arg=-lkernel32', '-C', 'link-arg=-lgcc']
            subprocess.run(cmd, check=True)
            if 'windows' not in target:
                subprocess.run([str(exe), str(root / 'vectors')], check=True, timeout=90)
            elif a.wine:
                subprocess.run(['wine', str(exe)], check=True, timeout=90, env=dict(
                    os.environ, WINEDEBUG='-all', WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
        original_timers = (here / 'timers.rs').read_text()
        for name, old, new in [
            ('round-relative-up', 'due.wrapping_neg() / 10000', '(due.wrapping_neg() + 9999) / 10000'),
            ('period-from-old-due', 'scan.now.wrapping_add(i64::from(period))', 'due.wrapping_add(i64::from(period))'),
            ('null-cancel', 'let mut was = false;', 'if timer == 0 { return Ok(false); }\n        let mut was = false;'),
            ('missing-fire-publication', 'self.timers_publish(version, bytes)?;\n        Ok(Some([timer, dpc]))', 'let _ = (version, bytes);\n        Ok(Some([timer, dpc]))'),
        ]:
            assert original_timers.count(old) == 1, name
            bad = root / (name + '.rs')
            bad.write_text(original_timers.replace(old, new))
            main_source = root / (name + '-main.rs')
            main_source.write_text(source.read_text().replace(
                json.dumps(str(here / 'timers.rs')), json.dumps(str(bad))))
            exe = root / name
            subprocess.run(['rustc', '--edition=2021', '-D', 'warnings', '-O',
                            str(main_source), '-C', 'link-arg=' + str(root / 'x86_64-unknown-linux-gnu.o'),
                            '-o', str(exe)], check=True)
            result = subprocess.run([str(exe)], capture_output=True, text=True, timeout=90)
            assert result.returncode != 0 and 'timer operation' in result.stderr, (name, result.stderr)
            print('PASS required failing ' + name + ' control; never admitted', flush=True)
        for name, path, old, new, failure in [
            ('premature-join-witness', here / '../service-kernel-dpc/queue.rs',
             'if !self.joined || self.worker == 0 {', 'if self.worker == 0 {', 'on an `Err` value: -3'),
            ('combined-clock-truncation', here / 'clock.rs',
             'Ok(host.wrapping_add(skew / 1000000) as i64)',
             'let _ = host; Ok(((counter / frequency).wrapping_mul(1000000000).wrapping_add((counter % frequency).wrapping_mul(1000000000) / frequency).wrapping_add(skew) / 1000000) as i64)',
             'clock case'),
        ]:
            text = path.read_text()
            assert text.count(old) == 1, name
            bad = root / (name + '.rs')
            bad.write_text(text.replace(old, new))
            main_source = root / (name + '-main.rs')
            main_source.write_text(source.read_text().replace(json.dumps(str(path)), json.dumps(str(bad))))
            exe = root / name
            subprocess.run(['rustc', '--edition=2021', '-D', 'warnings', '-O',
                            str(main_source), '-C', 'link-arg=' + str(root / 'x86_64-unknown-linux-gnu.o'),
                            '-o', str(exe)], check=True)
            result = subprocess.run([str(exe)], capture_output=True, text=True, timeout=90)
            assert result.returncode != 0 and failure in result.stderr, (name, result.stderr)
            print('PASS required failing ' + name + ' control; never admitted', flush=True)
        if a.app_lib:
            lib = a.app_lib.resolve(strict=True)
            spec = importlib.util.spec_from_file_location('helpers', here / '../service-save/check.py')
            helpers = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(helpers)
            exe = root / 'admission'
            cmd = ['rustc', '--edition=2021', '-D', 'warnings', str(here / 'admission.rs'),
                   '--extern', 'gamepkg=' + str(lib), '-L', 'dependency=' + str(lib.parent), '-o', str(exe)]
            for name in ['tempfile', 'toml', 'serde_json']:
                cmd += ['--extern', name + '=' + str(helpers.dependency(lib, name))]
            subprocess.run(cmd, check=True)
            subprocess.run([str(exe), str(app), str(here), str(root)], check=True)
        if a.evidence:
            a.evidence.mkdir(parents=True, exist_ok=False)
            shutil.copyfile(root / 'vectors', a.evidence / 'vectors.bin')
            receipts = {}
            for path in sorted(root.glob('receipt-*/services.json')):
                record = json.loads(path.read_text())
                assert len(record['game']['objects']) == len(record['raw_objects']) == 8
                name = path.parent.name + '.json'
                shutil.copyfile(path, a.evidence / name)
                receipts[name] = digest(path)
            if a.app_lib:
                assert len(receipts) == 4
            report = {'schema': 1, 'status': 'native-timer-table-component-not-gameplay',
                      'original': pins, 'sources': {name: digest(path) for name, path in modules},
                      'receipts': receipts, 'vectors_sha256': digest(root / 'vectors'),
                      'rustc': subprocess.check_output(['rustc', '-vV'], text=True),
                      'windows': 'Wine components' if a.wine else 'compile-only',
                      'profile_admission': 'blocked'}
            (a.evidence / 'qualification.json').write_text(json.dumps(report, indent=2) + '\n')
    print('Timer table only: shadow events, actual callbacks, timer loop/bootstrap and full source closure remain required.')


if __name__ == '__main__':
    main()
