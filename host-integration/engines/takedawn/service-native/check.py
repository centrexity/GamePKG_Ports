#!/usr/bin/env python3
"""Maintainer-only conversion/differential/admission checks. Never required by players."""
import argparse, hashlib, importlib.util, json, os
from pathlib import Path
import subprocess, tarfile, tempfile

KIT = "6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4"
UNIT = "prepared/src/recomp_0000_001.rs"
UNIT_HASH = "442f7812516f3c5157ee74e15aede23d0ac423105acb7d973733bbe9f01ad94d"

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app',type=Path,required=True)
    parser.add_argument('--app-lib',type=Path,required=True)
    parser.add_argument('--wine',action='store_true',help='also execute Windows comparison binaries under Wine')
    args=parser.parse_args()
    here=Path(__file__).resolve().parent;app=args.app.resolve(strict=True);library=args.app_lib.resolve(strict=True)
    # Reuse exact app fingerprint resolution, not the newest potentially different feature build.
    spec=importlib.util.spec_from_file_location('save_checks',here.parent/'service-save/check.py')
    helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
    target=library.parent.parent.parent
    env=dict(os.environ,CARGO_TARGET_DIR=str(target))
    manifest=here/'maintainer/Cargo.toml'
    subprocess.run(['cargo','test','--locked','--offline','--manifest-path',str(manifest)],env=env,check=True)
    subprocess.run(['cargo','build','--locked','--offline','--manifest-path',str(manifest)],env=env,check=True)
    tool=target/'debug/takedawn-safe-source-preparation'
    kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
    if hashlib.sha256(kit.read_bytes()).hexdigest()!=KIT:raise RuntimeError('Changed source kit requires review')
    with tempfile.TemporaryDirectory(prefix='gamepkg-native-source-') as name:
        scratch=Path(name)
        with tarfile.open(kit) as archive:
            original=archive.extractfile(UNIT).read()
        if hashlib.sha256(original).hexdigest()!=UNIT_HASH:raise RuntimeError('Changed original unit')
        unit=scratch/'original.rs';unit.write_bytes(original)
        generated=scratch/'generated'
        subprocess.run([str(tool),str(unit),str(generated)],check=True)
        for filename in ['game.rs','dispatch.rs','provenance.json']:
            if (generated/filename).read_bytes()!=(here/'generated'/filename).read_bytes():
                raise RuntimeError(f'Checked-in generated {filename} is stale')
        record=json.loads((generated/'provenance.json').read_text())
        if len(record['converted'])!=24 or len(record['rejected'])!=26:raise RuntimeError('Unexpected conversion coverage')
        changed=scratch/'changed.rs';changed.write_bytes(original+b'\n')
        denied=subprocess.run([str(tool),str(changed),str(scratch/'must-not-exist')],capture_output=True)
        assert denied.returncode!=0 and not (scratch/'must-not-exist').exists()
        probe=scratch/'admission'
        command=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern',f'gamepkg={library}','-L',f'dependency={library.parent}','-o',str(probe)]
        for name in ['cap_std','tempfile','toml']:
            command+=['--extern',f'{name}={helpers.dependency(library,name)}']
        subprocess.run(command,check=True)
        subprocess.run([str(probe),str(app),str(here),"--source-only"],check=True)
        names=record['converted']
        oracle=here.joinpath('oracle_prefix.inc.rs').read_text()+'\n'+generated.joinpath('oracle.rs').read_text()
        oracle+='\npub unsafe fn invoke(address:u32){match address{'+''.join(f'0x{n[4:]}=>{n}(),' for n in names)+'_=>panic!("unknown reference function")}}\n'
        oracle_file=scratch/'reference.rs';oracle_file.write_text(oracle)
        wrapper='#![allow(non_camel_case_types,non_snake_case,non_upper_case_globals,unused_assignments,unused_variables,unused_mut,unused_parens,dead_code,unreachable_code)]\n'
        for module,path in [('gamepkg',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here/'memory.rs'),('registers',here/'registers.rs'),('game',generated/'game.rs'),('dispatch',generated/'dispatch.rs'),('oracle',oracle_file)]:
            wrapper+=f'#[path={json.dumps(str(path))}]mod {module};\n'
        wrapper+=f'const ADDRESSES:[u32;{len(names)}]=['+','.join('0x'+n[4:] for n in names)+'];\n'
        wrapper+=here.joinpath('comparison.rs').read_text()
        main_rs=scratch/'main.rs';main_rs.write_text(wrapper)
        vectors=scratch/'vectors'
        for triple in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
            exe=scratch/('compare-'+triple+('.exe' if 'windows' in triple else ''))
            command=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(main_rs),'-o',str(exe)]
            if triple=='i686-unknown-linux-gnu':command+=['-C','linker=/usr/bin/gcc-11']
            subprocess.run(command,check=True)
            if 'windows' not in triple:
                subprocess.run([str(exe)]+([str(vectors)] if triple.startswith('x86_64') else []),check=True)
            elif args.wine:
                subprocess.run(['wine',str(exe)],check=True,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
            else:print(f'PASS {triple}: compilation only; runtime not exercised',flush=True)
        subprocess.run([str(probe),str(app),str(here),str(vectors)],check=True)
    print('Component evidence only: no complete generated game, ISO load, graphics, physical devices, audible audio or frame pacing.')
if __name__=='__main__':main()
