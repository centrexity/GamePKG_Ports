#!/usr/bin/env python3
"""Maintainer actual-SDK shared-worker join evidence; not whole-title qualification."""
import argparse, hashlib, importlib.util, json, os, shutil, subprocess, tempfile
from pathlib import Path

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args()
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent;tests=here/'join-tests'
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,relative in [('memory', '../service-native/memory.rs'), ('call_state', '../service-native/call_state.rs'), ('registers', '../service-native/registers.rs'), ('heap', '../service-heap/heap.rs'), ('contiguous', '../service-contiguous/contiguous.rs'), ('heap_service', '../service-heap/service.rs'), ('kernel_pool', '../service-kernel-pool/kernel_pool.rs'), ('wait_service', '../service-kernel-wait/wait.rs'), ('queue', '../service-kernel-dpc/queue.rs'), ('pacing', 'pacing.rs'), ('shared', 'shared.rs'), ('timers', '../service-kernel-timers/timers.rs'), ('timer_clock', '../service-kernel-timers/clock.rs'), ('probe', 'join-tests/probe.rs')]:modules.append((name,here/relative))
 with tempfile.TemporaryDirectory(prefix='gamepkg-shared-join-') as temp:
  root=Path(temp);source=root/'main.rs';source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+(tests/'runtime.rs').read_text());expected=None
  for target in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True);vector=root/(target+'.vectors')
   if 'windows' not in target:subprocess.run([str(exe),str(vector)],check=True,timeout=90)
   elif a.wine:subprocess.run(['wine',str(exe),'Z:'+str(vector).replace('/','\\')],check=True,timeout=90,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:continue
   data=vector.read_bytes();assert len(data)==48
   if expected is None:expected=data
   else:assert data==expected,target+' composition witnesses changed by target'
  (root/'vectors').write_bytes(expected)
  for name,path,old,new in [
   ('accept-wrong-schedule-job',here/'shared.rs','joined.task_id() != self.assigned','joined.task_id() != joined.task_id()'),
   ('accept-wrong-queue-job',here/'../service-kernel-dpc/queue.rs','joined.task_id() != self.worker','joined.task_id() != joined.task_id()'),
   ('close-unjoined-schedule',here/'shared.rs','if self.assigned != 0 && !self.joined {','if false {'),
  ]:
   text=path.read_text();assert text.count(old)==1,name
   bad=root/(name+'.rs');bad.write_text(text.replace(old,new));driver=root/(name+'-main.rs');driver.write_text(source.read_text().replace(json.dumps(str(path)),json.dumps(str(bad))))
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(driver),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=90)
   assert result.returncode!=0 and 'on an `Err` value: -3' in result.stderr,(name,result.stderr)
   print('PASS required failing '+name+' control; never admitted',flush=True)
  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here/'../service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
   exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(tests/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
   for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',a.evidence/'vectors.bin');receipts={}
   for path in sorted(root.glob('receipt-*/services.json')):
    record=json.loads(path.read_text());assert len(record['game']['objects'])==len(record['raw_objects'])==8
    name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=digest(path)
   if a.app_lib:assert len(receipts)==4
   report={'schema':1,'status':'shared-actual-worker-join-evidence-not-title-gameplay','sources':{name:digest(path) for name,path in modules},'receipts':receipts,'vectors_sha256':digest(root/'vectors'),'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows':'Wine components' if a.wine else 'compile-only','profile_admission':'blocked'}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('Shared actual join only; production timer/IRQ/bootstrap and title source/build/launch closure remain required.')
if __name__=='__main__':main()
