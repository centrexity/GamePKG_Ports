#!/usr/bin/env python3
"""Maintainer-only exact original kernel-data and certificate signature checks."""
import argparse,hashlib,hmac,importlib.util,json,os,re,subprocess,tarfile,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--wine',action='store_true');a=p.parse_args()
 app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent;pins=json.loads((here/'reference_inputs.json').read_text());heap_pins=json.loads((here.parent/'service-heap/reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
 assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']==heap_pins['source_kit_sha256']
 assert pins['zero_certificate_signature']==hmac.new(bytes.fromhex('5c0733ae0401f7e8ba7993fdcd2f1fe0'),bytes(16),'sha1').digest()[:16].hex()
 def body(data,pin):
  result=data[pin['start']:pin['end']];assert hashlib.sha256(result).hexdigest()==pin['sha256'];return result.decode()
 with tarfile.open(kit) as archive:
  heap=archive.extractfile(heap_pins['member']).read();assert hashlib.sha256(heap).hexdigest()==heap_pins['source_sha256']
  header=archive.extractfile(pins['header']['member']).read();assert hashlib.sha256(header).hexdigest()==pins['header']['sha256']
  text=(here.parent/'service-heap/heap_reference.c.in').read_text()
  for name,pin in heap_pins['functions'].items():text=text.replace('/* GAMEPKG_PINNED_'+name.upper()+' */',body(heap,pin))
  wrapper=(here/'reference.c.in').read_text()
  for entry in pins['sources'].values():
   data=archive.extractfile(entry['member']).read();assert hashlib.sha256(data).hexdigest()==entry['sha256']
   for name,pin in entry['functions'].items():wrapper=wrapper.replace('/* PINNED_'+name.upper()+' */',body(data,pin))
  definitions=re.findall(rb'(?m)^#define KDATA_[^\n]*',header);assert len(definitions)==27
  wrapper=wrapper.replace('/* PINNED_KDATA_DEFINITIONS */',b'\n'.join(definitions).decode())
  assert '/* PINNED_' not in wrapper and 'GAMEPKG_PINNED_' not in text
 modules=[('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('layout',here.parent/'service-memory/layout.rs'),('kernel_data',here/'kernel_data.rs'),('signature',here/'signature.rs'),('cases',here/'cases.rs'),('probe',here/'probe.rs')]
 with tempfile.TemporaryDirectory(prefix='gamepkg-kernel-data-') as temp:
  scratch=Path(temp);reference=scratch/'reference.c';reference.write_text(text+'\n'+wrapper);vectors=scratch/'vectors';source_rs=scratch/'main.rs';rs='#![allow(dead_code)]\n'
  for name,path in [('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),*modules]:rs+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  rs+='#[allow(unused_imports)]mod gamepkg{pub use crate::client::*;}\n'+(here/'reference.rs').read_text();source_rs.write_text(rs)
  for triple,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=scratch/(triple+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
   exe=scratch/(triple+('.exe' if 'windows' in triple else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(source_rs),'-C','link-arg='+str(obj),'-o',str(exe)]
   if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in triple:subprocess.run([str(exe),*([str(vectors)] if triple=='x86_64-unknown-linux-gnu' else [])],check=True,timeout=180)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=180,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+triple+': reference compilation only',flush=True)
  spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
  exe=scratch/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
  for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(vectors)],check=True)
 print('Kernel-data component only: not complete kernel import preparation, title entry or gameplay qualification.')
if __name__=='__main__':main()
