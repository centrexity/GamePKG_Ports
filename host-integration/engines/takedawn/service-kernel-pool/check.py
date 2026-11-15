#!/usr/bin/env python3
"""Maintainer-only exact pinned original pool bridges versus checked Rust."""
import argparse,hashlib,importlib.util,json,os,re,subprocess,tarfile,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--wine',action='store_true');p.add_argument('--app-lib',type=Path);p.add_argument('--call-targets',type=Path);a=p.parse_args();
 if bool(a.app_lib)!=bool(a.call_targets):p.error('--app-lib and --call-targets must be supplied together')
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent;kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';pins=json.loads((here/'reference_inputs.json').read_text());heap_pins=json.loads((here.parent/'service-heap/reference_inputs.json').read_text());assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']==heap_pins['source_kit_sha256']
 with tarfile.open(kit) as archive:source=archive.extractfile(pins['member']).read();heap_source=archive.extractfile(heap_pins['member']).read()
 assert hashlib.sha256(source).hexdigest()==pins['source_sha256'];assert hashlib.sha256(heap_source).hexdigest()==heap_pins['source_sha256']
 def body(data,pin):
  value=data[pin['start']:pin['end']];assert hashlib.sha256(value).hexdigest()==pin['sha256'];return value.decode()
 lookup=body(source,pins['lookup'])
 assert pins['ordinals']=={'14':'bridge_ExAllocatePool','15':'bridge_ExAllocatePoolWithTag','17':'bridge_ExFreePool','23':'bridge_ExQueryPoolBlockSize'}
 for ordinal,name in pins['ordinals'].items():assert len(re.findall(r'case\s+'+ordinal+r':\s*return\s+'+name+r'\s*;',lookup))==1
 with tempfile.TemporaryDirectory(prefix='gamepkg-pool-bridges-') as temp:
  scratch=Path(temp);text=(here.parent/'service-heap/heap_reference.c.in').read_text()
  for name,pin in heap_pins['functions'].items():text=text.replace('/* GAMEPKG_PINNED_'+name.upper()+' */',body(heap_source,pin))
  pool=(here/'pool_reference.c.in').read_text()
  for name,pin in pins['functions'].items():pool=pool.replace('/* GAMEPKG_PINNED_'+name.upper()+' */',body(source,pin))
  pool=pool.replace('/* GAMEPKG_PINNED_INIT_LOOP */',body(source,pins['init_loop']));assert 'GAMEPKG_PINNED_' not in text+pool;reference=scratch/'original.c';reference.write_text(text+'\n'+pool)
  wrapper='#![allow(dead_code)]\n'
  for name,path in [('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('registers',here.parent/'service-native/registers.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('kernel_pool',here/'kernel_pool.rs')]:wrapper+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  wrapper+='#[allow(unused_imports)]mod gamepkg{pub use crate::client::*;}\n'+(here/'reference.rs').read_text();source_rs=scratch/'main.rs';source_rs.write_text(wrapper)
  for triple,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=scratch/(triple+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
   exe=scratch/(triple+('.exe' if 'windows' in triple else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(source_rs),'-C','link-arg='+str(obj),'-o',str(exe)]
   if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11']
   subprocess.run(cmd,check=True)
   if 'windows' not in triple:subprocess.run([str(exe)],check=True,timeout=180)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=180,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+triple+': original reference compilation only',flush=True)
 if a.app_lib:
  lib=a.app_lib.resolve(strict=True);targets=a.call_targets.resolve(strict=True)
  spec=importlib.util.spec_from_file_location('service_reference_helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
  with tempfile.TemporaryDirectory(prefix='gamepkg-pool-admission-') as temp:
   probe=Path(temp)/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(probe)]
   for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(probe),str(app),str(here),str(targets)],check=True)
 print('Four pool handlers only. Full native-router admission/confinement runs in service-native/check.py; no title boot/gameplay qualification.')
if __name__=='__main__':main()
