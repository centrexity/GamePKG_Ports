#!/usr/bin/env python3
"""Maintainer-only pinned native IRQ composition differential (draft; races not qualified)."""
import argparse,hashlib,importlib.util,json,os,subprocess,tarfile,tempfile,shutil
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args();app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
 assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']
 with tarfile.open(kit) as t:
  sources={name:t.extractfile(pin['member']).read() for name,pin in pins['sources'].items()}
 for name,data in sources.items():assert hashlib.sha256(data).hexdigest()==pins['sources'][name]['sha256']
 text=(here/'reference.c.in').read_text()
 for name,pin in pins['parts'].items():
  body=sources[pin['source']][pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256'];text=text.replace('/* PINNED_'+name.upper()+' */',body.decode())
 assert '/* PINNED_' not in text
 mods=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('registers',here.parent/'service-native/registers.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs'),('gate',here.parent/'service-kernel-gate/gate.rs'),('irql',here.parent/'service-kernel-irql/state.rs'),('stacks',here.parent/'service-worker-stacks/stacks.rs'),('wait_service',here.parent/'service-kernel-wait/wait.rs'),('production_clock',here/'clock.rs'),('irq_delivery',here/'irq.rs'),('irq_hooks',here/'hooks.rs'),('irq_clock',here/'fixture_clock.rs'),('irq_routes',here/'fixture_routes.rs'),('irq_observer',here/'fixture_observer.rs'),('cases',here/'cases.rs'),('controls',here/'controls.rs'),('options',here/'options.rs'),('callbacks',here/'callbacks.rs'),('probe',here/'probe.rs')]
 with tempfile.TemporaryDirectory(prefix='gamepkg-irq-differential-') as temp:
  root=Path(temp);c=root/'reference.c';c.write_text(text);source=root/'main.rs'
  source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n'for name,path in mods)+(here/'reference.rs').read_text())
  for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(c),'-o',str(obj)],check=True)
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   if 'windows' in target:cmd+=['-C','link-arg=-lmingwex','-C','link-arg=-lmsvcrt','-C','link-arg=-lkernel32'] # Maintainer C snprintf diagnostics only.
   if target=='i686-pc-windows-gnu':cmd+=['-C','link-arg=-lgcc']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe),str(root/'vectors')],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+target+': compilation only',flush=True)
  # Restoring only the later bits drops the current exhausted line: must fail.
  current=(here/'irq.rs').read_text();needle='self.irq_restore_pending(ram, pending | bit, true)?;';assert current.count(needle)==1
  bad=root/'broken.rs';bad.write_text(current.replace(needle,'self.irq_restore_pending(ram, pending, true)?;'))
  wrapper=source.read_text().replace(json.dumps(str(here/'irq.rs')),json.dumps(str(bad)));negative=root/'negative.rs';negative.write_text(wrapper)
  obj=root/'negative.o';subprocess.run(['gcc','-std=c11','-O2','-c',str(c),'-o',str(obj)],check=True)
  exe=root/'negative';subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(negative),'-C','link-arg='+str(obj),'-o',str(exe)],check=True)
  result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=30);assert result.returncode!=0 and 'on an `Err` value: -3' in result.stderr,result.stderr
  print('PASS: required-failing exhausted-current-line restoration control; broken source never admitted',flush=True)
  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h);exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission_vectors.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
   for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(h.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False)
   shutil.copyfile(root/'vectors',a.evidence/'vectors.bin')
   receipts={}
   for receipt in sorted(root.glob('receipt-*/services.json')):
    data=json.loads(receipt.read_text());assert len(data['game']['objects'])==8 and len(data['raw_objects'])==8
    name=receipt.parent.name+'.json';shutil.copyfile(receipt,a.evidence/name);receipts[name]=hashlib.sha256(receipt.read_bytes()).hexdigest()
   if a.app_lib:assert len(receipts)==4
   sources={str(path.relative_to(here.parent)):hashlib.sha256(path.read_bytes()).hexdigest() for name,path in mods if name not in ['client','local']}
   report={'schema':1,'status':'default-composition-differential-qualified-not-post-races','source_kit_sha256':pins['source_kit_sha256'],'sources':sources,'receipts':receipts,'vectors_sha256':hashlib.sha256((root/'vectors').read_bytes()).hexdigest(),'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows_execution':'Wine' if a.wine else 'compile-only','profile_admission':'blocked','fixture_cases_per_target':128,'oracle_template_sha256':hashlib.sha256((here/'reference.c.in').read_bytes()).hexdigest(),'expanded_oracle_sha256':hashlib.sha256(text.encode()).hexdigest(),'reference_pins_sha256':hashlib.sha256((here/'reference_inputs.json').read_bytes()).hexdigest(),'signed32_wait_stat_reference_targets':['Linux32','Windows32','Windows64'],'long_wait_gameplay_limit':False}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('DRAFT: default IRQ composition differential only; posting races, options, production routes/timer/DPC and title entry still unqualified.')
if __name__=='__main__':main()
