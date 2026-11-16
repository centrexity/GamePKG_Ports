#!/usr/bin/env python3
"""Maintainer original-policy differential, lifecycle and source/object admission."""
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
    component = Path(__file__).resolve().parent
    here = component / 'notification-tests'
    pins = json.loads((here / 'reference_inputs.json').read_text())
    kit = component.parents[3] / 'takedawn/assets/takedawn-source-kit.tar.gz'
    assert digest(kit) == pins['source_kit_sha256']
    with tarfile.open(kit) as t:
        original = t.extractfile(pins['member']).read()
    assert hashlib.sha256(original).hexdigest() == pins['source_sha256']
    text = (here / 'reference.c.in').read_text()
    for name, pin in pins['functions'].items():
        body = original[pin['start']:pin['end']]
        assert hashlib.sha256(body).hexdigest() == pin['sha256']
        text = text.replace('/* PINNED_' + name.upper() + ' */', body.decode())
    assert '/* PINNED_' not in text
    modules = [('client', app / 'engines/native-services/client.rs'),
               ('local', app / 'engines/native-services/local_bindings.rs')]
    for name, path in [
        ('memory', 'service-native/memory.rs'),
        ('call_state', 'service-native/call_state.rs'),
        ('registers', 'service-native/registers.rs'),
        ('heap', 'service-heap/heap.rs'),
        ('contiguous', 'service-contiguous/contiguous.rs'),
        ('heap_service', 'service-heap/service.rs'),
        ('kernel_pool', 'service-kernel-pool/kernel_pool.rs'),
        ('wait_service', 'service-kernel-wait/wait.rs'),
        ('gate', 'service-kernel-gate/gate.rs'),
        ('irql', 'service-kernel-irql/state.rs'),
        ('queue', 'service-kernel-dpc/queue.rs'),
        ('notify', 'service-kernel-dpc/notify.rs'),
        ('irq_hooks', 'service-kernel-dpc/notification-tests/hooks.rs'),
        ('probe', 'service-kernel-dpc/notification-tests/probe.rs'),
    ]:
        modules.append((name, component.parent / path))
    with tempfile.TemporaryDirectory(prefix='gamepkg-dpc-notify-') as temp:
        root = Path(temp)
        reference = root / 'reference.c'
        reference.write_text(text)
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
            subprocess.run([compiler, *extra, '-std=c11', '-Wall', '-Wextra',
                            '-Werror', '-O2', '-c', str(reference), '-o', str(obj)], check=True)
            exe = root / (target + ('.exe' if 'windows' in target else ''))
            cmd = ['rustc', '--edition=2021', '-D', 'warnings', '-O', '--target',
                   target, str(source), '-C', 'link-arg=' + str(obj), '-o', str(exe)]
            if target == 'i686-unknown-linux-gnu':
                cmd += ['-C', 'linker=gcc-11', '-C', 'link-arg=-lssp_nonshared']
            subprocess.run(cmd, check=True)
            if 'windows' not in target:
                subprocess.run([str(exe), str(root / 'vectors')], check=True, timeout=90)
            elif a.wine:
                subprocess.run(['wine', str(exe)], check=True, timeout=90, env=dict(
                    os.environ, WINEDEBUG='-all', WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
            else:
                print('PASS ' + target + ': compile only', flush=True)
        # Regression controls are rejected maintainer builds, never admitted artifacts.
        original = (component / 'notify.rs').read_text()
        for name, old, new in [
            ('skip-isr', 'depth != 0 || values[4] != 0 || values[0] >= 2',
             'depth != 0 || values[0] >= 2'),
            ('pending-on-queued', 'if depth != 0 || !prompt {',
             'if depth != 0 || !prompt || !self.notify_live(ram,task)?.0.queue_pending(ram)? {'),
            ('global-depth', 'state[0] == task.task_key()[1]', 'true'),
        ]:
            assert original.count(old) == 1
            bad = root / (name + '.rs')
            bad.write_text(original.replace(old, new))
            main_source = root / (name + '-main.rs')
            main_source.write_text(source.read_text().replace(
                json.dumps(str(component / 'notify.rs')), json.dumps(str(bad))))
            exe = root / name
            subprocess.run(['rustc', '--edition=2021', '-D', 'warnings', '-O',
                            str(main_source), '-C', 'link-arg=' + str(root / 'x86_64-unknown-linux-gnu.o'),
                            '-o', str(exe)], check=True)
            result = subprocess.run([str(exe)], capture_output=True, text=True, timeout=90)
            assert result.returncode != 0, name
            assert 'DPC wake vector' in result.stderr or 'on an `Err` value: -3' in result.stderr, result.stderr
            print('PASS required failing ' + name + ' control; never admitted', flush=True)
        if a.app_lib:
            lib = a.app_lib.resolve(strict=True)
            spec = importlib.util.spec_from_file_location('helpers', component.parent / 'service-save/check.py')
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
            for receipt in sorted(root.glob('receipt-*/services.json')):
                record = json.loads(receipt.read_text())
                assert len(record['game']['objects']) == len(record['raw_objects']) == 8
                name = receipt.parent.name + '.json'
                shutil.copyfile(receipt, a.evidence / name)
                receipts[name] = digest(receipt)
            if a.app_lib:
                assert len(receipts) == 4
            report = {'schema': 1, 'status': 'notification-component-not-gameplay',
                      'original': pins, 'sources': {name: digest(path) for name, path in modules},
                      'receipts': receipts, 'vectors_sha256': digest(root / 'vectors'),
                      'rustc': subprocess.check_output(['rustc', '-vV'], text=True),
                      'windows': 'Wine component execution' if a.wine else 'compile-only',
                      'profile_admission': 'blocked'}
            (a.evidence / 'qualification.json').write_text(json.dumps(report, indent=2) + '\n')
    print('Notification component only; real DPC callback/timer/IRQ/device/gameplay integration remains required.')


if __name__ == '__main__':
    main()
