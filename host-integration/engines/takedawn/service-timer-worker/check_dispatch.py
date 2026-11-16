#!/usr/bin/env python3
"""Maintainer real-clock worker drain/fire composition; not full title admission."""
import argparse,hashlib,importlib.util,json,os,shutil,subprocess,tempfile
from pathlib import Path

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--app',type=Path,required=True);parser.add_argument('--app-lib',type=Path,required=True);parser.add_argument('--wine',action='store_true');parser.add_argument('--evidence',type=Path);a=parser.parse_args()
 app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,path in [('memory', '../service-native/memory.rs'), ('call_state', '../service-native/call_state.rs'), ('registers', '../service-native/registers.rs'), ('heap', '../service-heap/heap.rs'), ('contiguous', '../service-contiguous/contiguous.rs'), ('heap_service', '../service-heap/service.rs'), ('layout', '../service-memory/layout.rs'), ('kernel_pool', '../service-kernel-pool/kernel_pool.rs'), ('wait_service', '../service-kernel-wait/wait.rs'), ('gate', '../service-kernel-gate/gate.rs'), ('irql', '../service-kernel-irql/state.rs'), ('queue', '../service-kernel-dpc/queue.rs'), ('callback', '../service-kernel-dpc/callback.rs'), ('drain', '../service-kernel-dpc/drain.rs'), ('dpc_clock', '../service-kernel-dpc/clock.rs'), ('stacks', '../service-worker-stacks/stacks.rs'), ('shadow', '../service-kernel-shadow/shadow.rs'), ('events', '../service-kernel-shadow/events.rs'), ('timers', '../service-kernel-timers/timers.rs'), ('firing', '../service-kernel-timers/firing.rs'), ('timer_clock', '../service-kernel-timers/clock.rs'), ('production_clock', '../service-kernel-timers/clock.rs'), ('thread_tib', '../service-thread-tib/thread_tib.rs'), ('worker_setup', 'worker.rs'), ('dispatch', 'dispatch.rs'), ('dpc_observer', 'dispatch-tests/observer.rs'), ('irq_hooks', 'dispatch-tests/hooks.rs'), ('dpc_routes', 'dispatch-tests/routes.rs'), ('probe', 'dispatch-tests/probe.rs')]:modules.append((name,here/path))
 with tempfile.TemporaryDirectory(prefix='gamepkg-timer-worker-') as temp:
  root=Path(temp);driver=root/'main.rs';driver.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+(here/'dispatch-tests/runtime.rs').read_text());expected=None
  for target in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(driver),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True);vector=root/(target+'.vectors')
   if 'windows' not in target:subprocess.run([str(exe),str(vector)],check=True,timeout=90)
   elif a.wine:subprocess.run(['wine',str(exe),'Z:'+str(vector).replace('/','\\')],check=True,timeout=90,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:continue
   data=vector.read_bytes();assert len(data)==576
   if expected is None:expected=data
   else:assert data==expected,target+' composition witnesses changed by target'
  (root/'vectors').write_bytes(expected)
  text=(here/'dispatch.rs').read_text();worker=(here/'worker.rs').read_text()
  start=text.index('        let drained = self\n');end=text.index('        Ok([drained[0]',start)
  full=text[start:end];sample='        let now = production_clock::timer_now_ms(ram)?;\n';assert full.count(sample)==1
  fire_at=full.index('        let fired = ');drain_and_sample=full[:fire_at];drain_only=drain_and_sample.replace(sample,'');fire=full[fire_at:]
  for name,path,old,new in [
   ('fire-before-drain',here/'dispatch.rs',full,sample+fire+drain_only),
   ('clock-before-drain',here/'dispatch.rs',full,sample+drain_only+fire),
   ('accept-other-retained-stack',here/'dispatch.rs','lease.stack_identity(ram)? != self.stack','lease.stack_identity(ram)? != lease.stack_identity(ram)?'),
   ('close-other-retained-stack',here/'dispatch.rs','identity != self.stack','identity != identity'),
   ('accept-wrong-timer-queue',here/'dispatch.rs','table.timer_worker_binding(ram)? != [pending_id, actor]','table.timer_worker_binding(ram)? != table.timer_worker_binding(ram)?'),
   ('deny-cancelled-cleanup',here/'worker.rs','if gamepkg::local_task_status()?.0 != self.actor','if gamepkg::local_task_status()? != (self.actor, false)'),
  ]:
   original=path.read_text();assert original.count(old)==(2 if name=='deny-cancelled-cleanup' else 1),name
   bad=root/(name+'.rs');bad.write_text(original.replace(old,new,1));source=root/(name+'-main.rs');source.write_text(driver.read_text().replace(json.dumps(str(path)),json.dumps(str(bad))))
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(source),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=90)
   assert result.returncode!=0 and 'on an `Err` value:' in result.stderr,(name,result.stderr)
   print('PASS required failing '+name+' control; never admitted',flush=True)
  spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
  exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'dispatch-tests/admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
  for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',a.evidence/'vectors.bin');receipts={}
   for path in sorted(root.glob('receipt-*/services.json')):
    record=json.loads(path.read_text());assert len(record['game']['objects'])==len(record['raw_objects'])==8
    name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=digest(path)
   assert len(receipts)==4
   report={'schema':1,'status':'real-clock-native-worker-ordered-drain-fire-composition-not-title-gameplay','sources':{name:digest(path) for name,path in modules},'receipts':receipts,'vectors_sha256':digest(root/'vectors'),'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows':'Wine components' if a.wine else 'compile-only','profile_admission':'blocked'}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('Actual real-clock worker drain/fire composition only; production context, IRQ/profiler/renderer routes and whole timer loop/title closure remain required.')
if __name__=='__main__':main()
