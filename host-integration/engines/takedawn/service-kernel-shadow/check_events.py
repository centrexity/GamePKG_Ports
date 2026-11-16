#!/usr/bin/env python3
"""Maintainer original event-shadow adapter comparison; native events remain SDK-owned."""
import argparse, hashlib, importlib.util, json, os, shutil, subprocess, tarfile, tempfile
from pathlib import Path

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args()
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'event_reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
 assert digest(kit)==pins['source_kit_sha256']
 with tarfile.open(kit) as t:original=t.extractfile(pins['member']).read()
 assert hashlib.sha256(original).hexdigest()==pins['source_sha256']
 reference=(here/'event_reference.c.in').read_text()
 for name,pin in pins['functions'].items():
  body=original[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256'];reference=reference.replace('/* PINNED_'+name.upper()+' */',body.decode())
 assert '/* PINNED_' not in reference
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('wait_service','../service-kernel-wait/wait.rs'),('shadow','shadow.rs'),('events','events.rs'),('cases','cases.rs'),('event_probe','event_probe.rs'),('event_controls','event_controls.rs'),('event_workers','event_workers.rs')]:modules.append((name,here/relative))
 with tempfile.TemporaryDirectory(prefix='gamepkg-shadow-events-') as temp:
  root=Path(temp);c=root/'reference.c';c.write_text(reference);source=root/'main.rs';source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+(here/'event_runtime.rs').read_text())
  for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(c),'-o',str(obj)],check=True)
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   if 'windows' in target:cmd+=['-C','link-arg=-lmingwex','-C','link-arg=-lmsvcrt','-C','link-arg=-lkernel32']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe),str(root/'vectors')],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
  text=(here/'events.rs').read_text()
  for name,old,new in [
   ('noncanonical-timer-kind', 'event_replace(map, ram, guest, kind == 0, false)?;', 'event_replace(map, ram, guest, kind & 1 == 0, false)?;'),
   ('leak-replaced-event', 'event_release(old)?;', 'let _ = old;'),
   ('short-timer-header', 'ram.fill(guest, 0, 40)?;', 'ram.fill(guest, 0, 16)?;'),
   ('publish-before-signal', 'event_signal(id, true)?;\n            ram.write_u32(guest.checked_add(4).ok_or(-1)?, 1)?;', 'ram.write_u32(guest.checked_add(4).ok_or(-1)?, 1)?;\n            event_signal(id, true)?;'),
  ]:
   assert text.count(old)==1,name
   bad=root/(name+'.rs');bad.write_text(text.replace(old,new));driver=root/(name+'-main.rs');driver.write_text(source.read_text().replace(json.dumps(str(here/'events.rs')),json.dumps(str(bad))))
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(driver),'-C','link-arg='+str(root/'x86_64-unknown-linux-gnu.o'),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=120)
   assert result.returncode!=0 and ('event case' in result.stderr or 'on an `Err` value: -3' in result.stderr),(name,result.stderr)
   print('PASS required failing '+name+' control; never admitted',flush=True)
  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here/'../service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
   exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'event_admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
   for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',a.evidence/'vectors.bin');receipts={}
   for path in sorted(root.glob('receipt-*/services.json')):
    record=json.loads(path.read_text());assert len(record['game']['objects'])==len(record['raw_objects'])==8
    name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=digest(path)
   if a.app_lib:assert len(receipts)==4
   report={'schema':1,'status':'sdk-event-shadow-adapter-not-full-guest-waits-or-gameplay','original':pins,'sources':{name:digest(path) for name,path in modules},'receipts':receipts,'vectors_sha256':digest(root/'vectors'),'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows':'Wine components' if a.wine else 'compile-only','profile_admission':'blocked'}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('Event adapter only. Actual title callbacks, guest wait/alert/timeout semantics, timer loop/bootstrap and gameplay remain required.')
if __name__=='__main__':main()
