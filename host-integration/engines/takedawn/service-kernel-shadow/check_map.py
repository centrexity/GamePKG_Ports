#!/usr/bin/env python3
"""Maintainer original shadow-map comparison; synthetic tokens grant no native authority."""
import argparse, hashlib, importlib.util, json, os, shutil, subprocess, tarfile, tempfile
from pathlib import Path

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args()
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'map_reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
 assert digest(kit)==pins['source_kit_sha256']
 with tarfile.open(kit) as t:original=t.extractfile(pins['member']).read()
 assert hashlib.sha256(original).hexdigest()==pins['source_sha256']
 reference=(here/'map_reference.c.in').read_text()
 for name,pin in pins['functions'].items():
  body=original[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256'];reference=reference.replace('/* PINNED_'+name.upper()+' */',body.decode())
 assert '/* PINNED_' not in reference
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('wait_service','../service-kernel-wait/wait.rs'),('shadow','shadow.rs'),('events','events.rs'),('cases','cases.rs'),('map_probe','map_probe.rs'),('map_controls','map_controls.rs')]:modules.append((name,here/relative))
 with tempfile.TemporaryDirectory(prefix='gamepkg-shadow-map-') as temp:
  root=Path(temp);c=root/'reference.c';c.write_text(reference);source=root/'main.rs';source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+(here/'map_runtime.rs').read_text())
  for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(c),'-o',str(obj)],check=True)
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   if 'windows' in target:cmd+=['-C','link-arg=-lmingwex','-C','link-arg=-lmsvcrt','-C','link-arg=-lkernel32']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe),str(root/'vectors')],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
  mutated_path=here/'../service-kernel-pool/kernel_pool.rs'
  text=mutated_path.read_text()
  for name,old,new in [
   ('early-close', 'if shadow_word(&bytes, 32)? != 0 {', 'if false {'),
   ('wrong-page-owner', 'if shadow_word(&bytes, 8)? != record {', 'if record == 0 {'),
   ('missing-slot-removal', 'shadow_put32(&mut bytes, 16 + slot * 12, 0)?;', 'shadow_put32(&mut bytes, 16 + slot * 12, guest)?;'),
  ]:
   assert text.count(old)==1,name
   bad=root/(name+'.rs');bad.write_text(text.replace(old,new));driver=root/(name+'-main.rs');driver.write_text(source.read_text().replace(json.dumps(str(mutated_path)),json.dumps(str(bad))))
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(driver),'-C','link-arg='+str(root/'x86_64-unknown-linux-gnu.o'),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=120)
   assert result.returncode!=0 and 'on an `Err` value:' in result.stderr,(name,result.stderr)
   print('PASS required failing '+name+' control; never admitted',flush=True)
  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here/'../service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
   exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'map_admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
   for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',a.evidence/'vectors.bin');receipts={}
   for path in sorted(root.glob('receipt-*/services.json')):
    record=json.loads(path.read_text());assert len(record['game']['objects'])==len(record['raw_objects'])==8
    name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=digest(path)
   if a.app_lib:assert len(receipts)==4
   report={'schema':1,'status':'logical-shadow-map-not-native-events-or-gameplay','original':pins,'sources':{name:digest(path) for name,path in modules},'receipts':receipts,'vectors_sha256':digest(root/'vectors'),'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows':'Wine components' if a.wine else 'compile-only','profile_admission':'blocked'}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('Logical map only. Synthetic metadata tokens do not qualify native event ownership, timer delivery, waits or title gameplay.')
if __name__=='__main__':main()
