#!/usr/bin/env python3
"""Maintainer-only original DPC queue differential; no player Python requirement."""
import argparse, hashlib, importlib.util, json, os, subprocess, tarfile, tempfile
from pathlib import Path
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');a=p.parse_args()
    app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent;pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
    assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']
    with tarfile.open(kit) as t:data=t.extractfile(pins['member']).read()
    assert hashlib.sha256(data).hexdigest()==pins['source_sha256']
    text=(here/'reference.c.in').read_text()
    for name,pin in pins['parts'].items():
        body=data[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256'];text=text.replace('/* PINNED_'+name.upper()+' */',body.decode())
    assert '/* PINNED_' not in text
    with tempfile.TemporaryDirectory(prefix='gamepkg-dpc-') as temp:
        root=Path(temp);c=root/'reference.c';c.write_text(text)
        modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('registers',here.parent/'service-native/registers.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs'),('wait_service',here.parent/'service-kernel-wait/wait.rs'),('queue',here/'queue.rs'),('cases',here/'cases.rs'),('probe',here/'probe.rs')]
        source=root/'runtime.rs';source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+(here/'runtime.rs').read_text())
        for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
            obj=root/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(c),'-o',str(obj)],check=True)
            exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-C','link-arg='+str(obj),'-o',str(exe)]
            if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
            if 'windows' in target:cmd+=['-C','link-arg=-lmsvcrt']
            subprocess.run(cmd,check=True)
            for stats in [0,1]:
                if 'windows' not in target:subprocess.run([str(exe),str(stats),str(root/('vectors-'+str(stats)))],check=True,timeout=60)
                elif a.wine:subprocess.run(['wine',str(exe),str(stats)],check=True,timeout=60,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
                else:print('PASS '+target+': compilation only',flush=True)
        current=(here/'queue.rs').read_text();needle='queue_put32(&mut bytes, at, 0)?;';assert current.count(needle)==1
        bad=root/'regressed_queue.rs';bad.write_text(current.replace(needle,'queue_put32(&mut bytes, at + 4, 0)?;'))
        wrapper=source.read_text();original=json.dumps(str(here/'queue.rs'));assert wrapper.count(original)==1
        negative=root/'negative.rs';negative.write_text(wrapper.replace(original,json.dumps(str(bad))))
        obj=root/'negative.o';subprocess.run(['gcc','-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(c),'-o',str(obj)],check=True)
        exe=root/'negative';subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(negative),'-C','link-arg='+str(obj),'-o',str(exe)],check=True)
        result=subprocess.run([str(exe),'0'],capture_output=True,text=True,timeout=60)
        assert result.returncode!=0 and 'vector 65, stats 0' in result.stderr,result.stderr
        print('PASS: required failing canceled-hole regression is detected by original-C full-ring witness, never admitted game code',flush=True)
        if a.app_lib:
            lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
            exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
            for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
            subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
    print('Queue component only: not dispatch gates, IRQL, callbacks, timer/interrupt integration or gameplay.')
if __name__=='__main__':main()
