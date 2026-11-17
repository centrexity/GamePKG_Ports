#!/usr/bin/env python3
"""Maintainer-only pinned original-C registration/ISR conformance; no player-side tools."""
import argparse,hashlib,importlib.util,json,os,shutil,subprocess,tarfile,tempfile
from pathlib import Path

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--app',type=Path,required=True);parser.add_argument('--app-lib',type=Path);parser.add_argument('--wine',action='store_true');parser.add_argument('--evidence',type=Path);a=parser.parse_args()
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent;kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';pins=json.loads((here/'reference_inputs.json').read_text());assert digest(kit)==pins['source_kit_sha256'];reference=(here/'reference.c.in').read_text()
 with tarfile.open(kit) as archive:
  for name,entry in pins['files'].items():
   data=archive.extractfile(name).read();assert hashlib.sha256(data).hexdigest()==entry['sha256']
   if name!='toolkit/src/kernel/kernel_bridge.c':continue
   for name,pin in entry['functions'].items():
    value=data[pin['start']:pin['end']];assert hashlib.sha256(value).hexdigest()==pin['sha256']
    marker='/* PINNED_'+name.upper()+' */'
    if marker in reference:reference=reference.replace(marker,value.decode())
 assert '/* PINNED_' not in reference
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,path in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('layout','../service-memory/layout.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('gate','../service-kernel-gate/gate.rs'),('irql','../service-kernel-irql/state.rs'),('stacks','../service-worker-stacks/stacks.rs'),('thread_tib','../service-thread-tib/thread_tib.rs'),('worker_setup','../service-timer-worker/worker.rs'),('irq_hooks','../service-kernel-dpc/callback-tests/hooks.rs'),('connections','connections.rs'),('isr_call','call.rs'),('cases','cases.rs'),('isr_routes','routes.rs'),('isr_observer','observer.rs'),('probe','probe.rs'),('faults','faults.rs')]:modules.append((name,here/path))
 with tempfile.TemporaryDirectory(prefix='gamepkg-isr-conformance-') as temp:
  root=Path(temp);c=root/'reference.c';c.write_text(reference);driver=root/'main.rs';driver.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+(here/'reference.rs').read_text());expected=None
  for target,compiler,flags in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(target+'.o');subprocess.run([compiler,*flags,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(c),'-o',str(obj)],check=True)
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(driver),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   if 'windows' in target:cmd+=['-C','link-arg=-lmsvcrt'] # Maintainer C assertions only, after oracle object.
   subprocess.run(cmd,check=True);vector=root/(target+'.vectors')
   if 'windows' not in target:subprocess.run([str(exe),str(vector)],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe),'Z:'+str(vector).replace('/','\\')],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:continue
   data=vector.read_bytes();assert len(data)==35968
   if expected is None:expected=data
   else:assert data==expected,target+' original witnesses changed by target'
  (root/'vectors').write_bytes(expected)
  replacements=[
   ('full-eax-instead-of-bit',here/'call.rs','Ok((cpu.eax & 1) as i32)','Ok(cpu.eax as i32)',1),
   ('swapped-isr-arguments',here/'call.rs','[context, object, 0u32]','[object, context, 0u32]',1),
   ('accept-bad-stdcall',here/'call.rs','native.is_ok() && cpu.esp == returned','native.is_ok() && returned == returned',1),
   ('suppress-profile-end-error',here/'call.rs','Ok(()) => ended','Ok(()) => { let _ = ended; Ok(()) }',1),
   ('skip-fatal-abort',here/'call.rs','if task.irql_abort(ram).is_err()','if false',1),
   ('omit-manual-route',here/'routes.rs','routine == 0x1100 || routine == 0x2200','routine == 0x1100 || routine == 0x1100',1),
   ('drop-vector-31',here/'connections.rs','if vector >= 32 {','if vector >= 31 {',2),
   ('borrowed-close-allowed',here/'connections.rs','if !self.owned {','if false {',1),
  ]
  for name,path,old,new,count in replacements:
   original=path.read_text();assert original.count(old)==count,name
   bad=root/(name+'.rs');bad.write_text(original.replace(old,new,1));source=root/(name+'-main.rs');source.write_text(driver.read_text().replace(json.dumps(str(path)),json.dumps(str(bad))))
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(source),'-C','link-arg='+str(root/'x86_64-unknown-linux-gnu.o'),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=120)
   assert result.returncode!=0 and ('on an `Err` value:' in result.stderr or 'original ISR case' in result.stderr),(name,result.stderr)
   print('PASS required failing '+name+' control; never admitted',flush=True)
  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
   exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
   for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',a.evidence/'vectors.bin');receipts={}
   for path in sorted(root.glob('receipt-*/services.json')):
    value=json.loads(path.read_text());assert len(value['game']['objects'])==len(value['raw_objects'])==8
    name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=digest(path)
   if a.app_lib:assert len(receipts)==4
   report={'schema':1,'status':'original-C-ISR-convention-and-native-fatal-prefix-component-not-title','original':pins,'receipts':receipts,'sources':{name:digest(path) for name,path in modules},'vectors_sha256':digest(root/'vectors'),'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows':'Wine components' if a.wine else 'compile-only','profile_admission':'blocked'}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('Original ISR convention and native fatal-prefix component only; actual device/profiler/context, whole loop and full title source/build/launcher closure remain required.')
if __name__=='__main__':main()
