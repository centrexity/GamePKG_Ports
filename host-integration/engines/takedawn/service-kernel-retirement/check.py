#!/usr/bin/env python3
"""Maintainer actual-cancelled creator retirement; not a title admission tool."""
import argparse,hashlib,json,os,subprocess,tempfile
from pathlib import Path
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path,required=True);p.add_argument('--implementation',type=Path,help='Maintainer-only altered-core experiment; no game build path');a=p.parse_args();app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent
 implementation=a.implementation.resolve(strict=True)if a.implementation else(here/'../service-kernel-pool/kernel_pool.rs').resolve()
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('setup','../service-kernel-mixed-waits/setup.rs')]:modules.append((name,implementation if name=='kernel_pool'else(here/relative).resolve()))
 def declarations(paths):return '#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n'for name,path in paths)
 sources={name:sha(path)for name,path in modules};tests={name:sha(here/name)for name in ['runtime.rs','check.py']};out=a.evidence.resolve();out.mkdir(exist_ok=False);results=[]
 with tempfile.TemporaryDirectory(prefix='gamepkg-creator-retirement-')as temp:
  root=Path(temp);source=root/'driver.rs';source.write_text(declarations(modules)+(here/'runtime.rs').read_text())
  for triple in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   exe=root/(triple+('.exe'if'windows'in triple else''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(source),'-o',str(exe)];log=out/(triple+'.log')
   if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   execute='windows'not in triple or a.wine
   with log.open('w')as f:
    subprocess.run(cmd,check=True,stdout=f,stderr=subprocess.STDOUT)
    if execute:
     for mode in [1,3]:subprocess.run([*(['wine',str(exe)]if'windows'in triple else[str(exe)]),str(mode)],check=True,stdout=f,stderr=subprocess.STDOUT,timeout=30,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   results.append({'target':triple,'execution':'Wine'if execute and'windows'in triple else'native Linux'if execute else'build-only','log_sha256':sha(log)});print('PASS actual creator cancellation/native retirement '+triple,flush=True)
  full=implementation.read_text();broken=[]
  for name,kind in [('events',1),('semaphores',3)]:
   start=full.index('    fn '+name+'_drain_row(');stop=full.index('    pub fn '+name+'_close(',start);body=full[start:stop]
   old='''        match gamepkg::local_sync_release(id, '''+str(kind)+''') {
            Ok(()) | Err(-4) => {}
            Err(code) => return Err(code),
        }'''
   new='''        match gamepkg::local_'''+('event'if kind==1 else'semaphore')+'''_state(id) {
            Ok(_) => '''+name+'''_release(id)?,
            Err(-4) => {}
            Err(code) => return Err(code),
        }'''
   assert body.count(old)==1;bad=root/(name+'-cancelled-query.rs');bad.write_text(full[:start]+body.replace(old,new,1)+full[stop:]);driver=root/(name+'-main.rs');driver.write_text(declarations([(n,bad if n=='kernel_pool'else path)for n,path in modules])+(here/'runtime.rs').read_text());exe=root/name;log=out/(name+'-required-failing.log')
   with log.open('w')as f:
    subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(driver),'-o',str(exe)],check=True,stdout=f,stderr=subprocess.STDOUT);r=subprocess.run([str(exe),str(kind)],stdout=f,stderr=subprocess.STDOUT,timeout=30)
   assert r.returncode!=0 and'typed retirement must remove the actual object'in log.read_text(),name
   broken.append(name);print('PASS required-failing cancelled query mistaken for retirement '+name,flush=True)
 assert sources=={name:sha(path)for name,path in modules}and tests=={name:sha(here/name)for name in tests},'sources changed during qualification'
 (out/'qualification.json').write_text(json.dumps({'schema':1,'status':'passed','sources':sources,'test_sources':tests,'results':results,'required_failing_native_copies':broken,'scope':'maintainer-only deterministic query interference triggers actual public self-cancellation after cleanup guard acquisition; actual SDK/native jobs, no forged objects or cancellation bits','limits':'only event/semaphore domain/native retirement proven; cancelled creator retains root/table obligations for outer runtime teardown; no signed provider admission, title entry, physical Windows controls/audio/GPU/frame pacing'},indent=2)+'\n')
if __name__=='__main__':main()
