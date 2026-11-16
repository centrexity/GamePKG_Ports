#!/usr/bin/env python3
"""Maintainer actual-SDK timer/callback/event composition; not whole-title qualification."""
import argparse, hashlib, importlib.util, json, os, shutil, subprocess, tempfile
from pathlib import Path

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args()
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent;tests=here/'firing-tests'
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('wait_service','../service-kernel-wait/wait.rs'),('gate','../service-kernel-gate/gate.rs'),('irql','../service-kernel-irql/state.rs'),('queue','../service-kernel-dpc/queue.rs'),('callback','../service-kernel-dpc/callback.rs'),('stacks','../service-worker-stacks/stacks.rs'),('shadow','../service-kernel-shadow/shadow.rs'),('events','../service-kernel-shadow/events.rs'),('timers','timers.rs'),('firing','firing.rs'),('timer_clock','firing-tests/clock.rs'),('production_clock','clock.rs'),('callback_cases','../service-kernel-dpc/callback-tests/cases.rs'),('dpc_observer','../service-kernel-dpc/callback-tests/observer.rs'),('irq_hooks','../service-kernel-dpc/callback-tests/hooks.rs'),('dpc_routes','firing-tests/routes.rs'),('probe','firing-tests/probe.rs')]:modules.append((name,here/relative))
 with tempfile.TemporaryDirectory(prefix='gamepkg-timer-firing-') as temp:
  root=Path(temp);source=root/'main.rs';source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+(tests/'runtime.rs').read_text());expected=None
  for target in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True);vector=root/(target+'.vectors')
   if 'windows' not in target:subprocess.run([str(exe),str(vector)],check=True,timeout=90)
   elif a.wine:subprocess.run(['wine',str(exe),'Z:'+str(vector).replace('/','\\')],check=True,timeout=90,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:continue
   data=vector.read_bytes();assert len(data)==9*512
   if expected is None:expected=data
   else:assert data==expected,target+' composition witnesses changed by target'
  (root/'vectors').write_bytes(expected)
  text=(here/'firing.rs').read_text()
  invoke='invoker.callback_run(ram, task, cpu, lease, entry[1], 0, 0, profile)?;'
  held='let b=table.timers_state(ram)?;let lock=u64::from_le_bytes([b[24],b[25],b[26],b[27],b[28],b[29],b[30],b[31]]);gamepkg::local_mutex_lock(lock,0)?;'+invoke+'gamepkg::local_call(24,&u64::to_le_bytes(lock),&mut [])?;'
  for name,old,new,expected_failure in [
   ('signal-before-callback', 'if entry[1] != 0 {', 'events::timer_signal_after_callback(map_view,ram,entry[0])?; if entry[1] != 0 {', '-3'),
   ('skip-queue-binding', 'binding[0] != invoker.callback_queue_id()', 'binding[0] != binding[0]', '-3'),
   ('ignore-signal-error', 'events::timer_signal_after_callback(map_view, ram, entry[0])?;', 'let _=events::timer_signal_after_callback(map_view,ram,entry[0]);', '-3'),
   ('skip-fatal-abort', 'if task.irql_abort(ram).is_err()', 'if false', '-3'),
   ('hold-timer-lock', invoke, held, '-2'),
  ]:
   assert text.count(old)==1,name
   bad=root/(name+'.rs');bad.write_text(text.replace(old,new));driver=root/(name+'-main.rs');driver.write_text(source.read_text().replace(json.dumps(str(here/'firing.rs')),json.dumps(str(bad))))
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(driver),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=90)
   assert result.returncode!=0 and ('on an `Err` value: '+expected_failure) in result.stderr,(name,result.stderr)
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
   report={'schema':1,'status':'native-timer-callback-event-composition-synthetic-routes-not-whole-loop-or-title-fidelity','sources':{name:digest(path) for name,path in modules},'receipts':receipts,'vectors_sha256':digest(root/'vectors'),'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows':'Wine components' if a.wine else 'compile-only','profile_admission':'blocked'}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('Actual source-checked native composition only. Production finite handlers/profiler, whole timer loop, cancellation/waits and title source/build/launch closure remain required.')
if __name__=='__main__':main()
