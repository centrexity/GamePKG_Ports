#!/usr/bin/env python3
"""Maintainer-only original host_time differential and checked sleeper qualification."""
import argparse, hashlib, importlib.util, json, os, subprocess, tarfile, tempfile
from pathlib import Path

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app',type=Path,required=True)
    p.add_argument('--app-lib',type=Path)
    p.add_argument('--wine',action='store_true')
    p.add_argument('--repeat',type=int,choices=range(1,65),default=1)
    a=p.parse_args();app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent
    pins=json.loads((here/'reference_inputs.json').read_text())
    kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
    assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']
    with tarfile.open(kit) as t:data=t.extractfile(pins['member']).read()
    assert hashlib.sha256(data).hexdigest()==pins['source_sha256']
    text=(here/'reference.c.in').read_text()
    for key,pin in pins['functions'].items():
        body=data[pin['start']:pin['end']]
        assert hashlib.sha256(body).hexdigest()==pin['sha256']
        text=text.replace('/* PINNED_'+key.upper()+' */',body.decode())
    assert '/* PINNED_' not in text
    modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('registers',here.parent/'service-native/registers.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs'),('wait_service',here/'wait.rs'),('cases',here/'cases.rs'),('probe',here/'probe.rs')]
    with tempfile.TemporaryDirectory(prefix='gamepkg-wait-') as temp:
        root=Path(temp);reference=root/'reference.c';reference.write_text(text)
        rs='#![allow(dead_code)]\n'
        for name,path in modules:rs+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
        source=root/'main.rs';source.write_text(rs+(here/'reference.rs').read_text());vectors=root/'vectors'
        for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
            obj=root/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
            exe=root/(target+('.exe' if 'windows' in target else ''))
            cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-C','link-arg='+str(obj),'-o',str(exe)]
            if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
            if target=='i686-pc-windows-gnu':cmd+=['-C','link-arg=-lgcc']
            subprocess.run(cmd,check=True)
            env=dict(os.environ,GAMEPKG_WAIT_REPEATS=str(a.repeat))
            env.pop('GAMEPKG_WAIT_REGRESSION_ONLY',None)
            if 'windows' not in target:subprocess.run([str(exe),*([str(vectors)] if target.startswith('x86_64') else [])],check=True,timeout=180,env=env)
            elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=180,env=dict(env,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
            else:print('PASS '+target+': compilation only',flush=True)
        current=(here/'wait.rs').read_text()
        needle='let values = kernel_pool::kernel_service_values(ram);'
        assert current.count(needle)==1
        bad=root/'regressed_wait.rs';bad.write_text(current.replace(needle,needle+'\n        values?;'))
        original_path=json.dumps(str(here/'wait.rs'));assert rs.count(original_path)==1
        negative=root/'negative.rs';negative.write_text(rs.replace(original_path,json.dumps(str(bad)))+(here/'reference.rs').read_text())
        binary=root/'negative';subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(negative),'-C','link-arg='+str(root/'x86_64-unknown-linux-gnu.o'),'-o',str(binary)],check=True)
        rejected=subprocess.run([str(binary)],env=dict(os.environ,GAMEPKG_WAIT_REGRESSION_ONLY='1'),capture_output=True,text=True,timeout=30)
        assert rejected.returncode!=0 and 'left: Err(-5)' in rejected.stderr and 'right: Err(-4)' in rejected.stderr,rejected.stderr
        print('PASS: deterministic revocation control rejects the old early-error propagation; regressed source is maintainer-only',flush=True)
        if a.app_lib:
            lib=a.app_lib.resolve(strict=True)
            spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
            exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
            for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
            subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(vectors)],check=True)
    print('Clock/wake adapter only: no full kernel timer/DPC/IRQ scheduler, title entry, graphics, physical inputs, audible audio or gameplay pacing.')
if __name__=='__main__':main()
