#!/usr/bin/env python3
"""Maintainer tick-word differential and checked worker qualification, never player tooling."""
import argparse,hashlib,importlib.util,json,os,subprocess,tarfile,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');a=p.parse_args()
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent;pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']
 with tarfile.open(kit) as t:data=t.extractfile(pins['member']).read()
 assert hashlib.sha256(data).hexdigest()==pins['source_sha256']
 for part in ['function','statement']:
  pin=pins[part];body=data[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256']
 assert pins['function']['start']<=pins['statement']['start']<pins['statement']['end']<=pins['function']['end']
 text=(here/'reference.c.in').read_text().replace('/* PINNED_TICK_STATEMENT */',data[pins['statement']['start']:pins['statement']['end']].decode())
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('registers',here.parent/'service-native/registers.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs'),('kernel_data',here.parent/'service-kernel-data/kernel_data.rs'),('signature',here.parent/'service-kernel-data/signature.rs'),('tick_service',here/'tick.rs'),('probe',here/'probe.rs')]
 with tempfile.TemporaryDirectory(prefix='gamepkg-tick-') as temp:
  root=Path(temp);reference=root/'reference.c';reference.write_text(text)
  rs='#![allow(dead_code)]\n'
  for name,path in modules:rs+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  source=root/'main.rs';source.write_text(rs+(here/'reference.rs').read_text())
  for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe)],check=True,timeout=180)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=180,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+target+': compilation only')
  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
   exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
   for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here)],check=True)
 print('Periodic tick component only; no complete title, GPU/DSP/IRQ/timer/DPC scheduler, physical input, audible audio or frame-pacing qualification.')
if __name__=='__main__':main()
