#!/usr/bin/env python3
"""Maintainer-only original display/vblank state differential; never player tooling."""
import argparse, hashlib, importlib.util, json, os, subprocess, tarfile, tempfile, shutil
from pathlib import Path
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);p.add_argument('--repeat',type=int,choices=range(1,33),default=1);a=p.parse_args()
    app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent;pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
    assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']
    with tarfile.open(kit) as t:data=t.extractfile(pins['member']).read()
    assert hashlib.sha256(data).hexdigest()==pins['source_sha256']
    text=(here/'reference.c.in').read_text()
    for name,pin in [('globals',pins['globals']),*pins['functions'].items()]:
        body=data[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256'];text=text.replace('/* PINNED_'+name.upper()+' */',body.decode())
    assert '/* PINNED_' not in text
    with tempfile.TemporaryDirectory(prefix='gamepkg-pacing-') as temp:
        root=Path(temp);c=root/'reference.c';c.write_text(text);source=root/'main.rs'
        source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(here/(name+'.rs')))+']mod '+name+';\n' for name in ['pacing','cases'])+(here/'reference.rs').read_text())
        for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
            obj=root/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(c),'-o',str(obj)],check=True)
            exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-C','link-arg='+str(obj),'-o',str(exe)]
            if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
            if target=='i686-pc-windows-gnu':cmd+=['-C','link-arg=-lgcc']
            if 'windows' in target:cmd+=['-C','link-arg=-lmsvcrt'] # Maintainer C string diagnostics only, after the oracle object.
            subprocess.run(cmd,check=True)
            for mode in [-1,0,1]:
                if 'windows' not in target:subprocess.run([str(exe),str(mode),str(root/('vectors-'+str(mode)))],check=True,timeout=60)
                elif a.wine:subprocess.run(['wine',str(exe),str(mode)],check=True,timeout=60,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
                else:print('PASS '+target+': compilation only',flush=True)
        modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('registers',here.parent/'service-native/registers.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs'),('wait_service',here.parent/'service-kernel-wait/wait.rs'),('pacing',here/'pacing.rs'),('shared',here/'shared.rs'),('probe',here/'probe.rs')]
        runtime=root/'runtime.rs';runtime.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+(here/'runtime.rs').read_text())
        for target in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
            exe=root/('runtime-'+target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(runtime),'-o',str(exe)]
            if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
            subprocess.run(cmd,check=True)
            env=dict(os.environ,GAMEPKG_PACING_REPEATS=str(a.repeat));env.pop('GAMEPKG_PACING_REGRESSION_ONLY',None)
            if 'windows' not in target:subprocess.run([str(exe)],check=True,timeout=120,env=env)
            elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=120,env=dict(env,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
            else:print('PASS '+target+': runtime compilation only',flush=True)
        current=(here/'shared.rs').read_text();needle='let sampled = wait_service::guest_now_ns(ram);';assert current.count(needle)==1
        bad=root/'regressed_shared.rs';bad.write_text(current.replace(needle,needle+'\n        sampled?;'))
        original=json.dumps(str(here/'shared.rs'));wrapper=runtime.read_text();assert wrapper.count(original)==1
        negative=root/'negative_runtime.rs';negative.write_text(wrapper.replace(original,json.dumps(str(bad))))
        exe=root/'negative';subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(negative),'-o',str(exe)],check=True)
        result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=30,env=dict(os.environ,GAMEPKG_PACING_REGRESSION_ONLY='1'))
        assert result.returncode!=0 and 'left: Err(-5)' in result.stderr and 'right: Err(-4)' in result.stderr,result.stderr
        print('PASS: deterministic mid-clock revocation control rejects old premature error propagation; never admitted game code',flush=True)
        if a.app_lib:
            lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
            exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
            for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
            subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
        if a.evidence:
            a.evidence.mkdir(parents=True,exist_ok=False);receipts={}
            for path in sorted(root.glob('receipt-*/services.json')):
                value=json.loads(path.read_text());assert len(value['game']['objects'])==len(value['raw_objects']) and len(value['raw_objects'])>=1
                name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=hashlib.sha256(path.read_bytes()).hexdigest()
            if a.app_lib:assert len(receipts)==4
            vectors={}
            for mode in [-1,0,1]:
                path=root/('vectors-'+str(mode));name=path.name+'.bin';shutil.copyfile(path,a.evidence/name);vectors[name]=hashlib.sha256(path.read_bytes()).hexdigest()
            report={'schema':1,'status':'pacing-state-and-scheduling-component-not-hardware-cadence-or-gameplay','original':pins,'receipts':receipts,'vectors':vectors,'sources':{name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in modules},'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows':'Wine components' if a.wine else 'compile-only','profile_admission':'blocked'}
            (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Pacing state component only; not actual display reports, IRQ delivery, complete timer/DPC execution, gameplay or measured frame pacing.')
if __name__=='__main__':main()
