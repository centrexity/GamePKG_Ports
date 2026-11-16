#!/usr/bin/env python3
"""Maintainer-only exact original NV settings/kernel service bridges versus checked Rust."""
import argparse,hashlib,importlib.util,json,os,re,subprocess,tarfile,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');a=p.parse_args()
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent;kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';pins=json.loads((here/'reference_inputs.json').read_text());assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']
 text=(here/'reference.c.in').read_text()
 with tarfile.open(kit) as archive:
  for member,entry in pins['headers'].items():
   data=archive.extractfile(member).read();assert hashlib.sha256(data).hexdigest()==entry['sha256']
   for name,pin in entry['definitions'].items():
    body=data[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256'];text=text.replace('/* PINNED_'+name+' */',body.decode())
  for member,entry in pins['sources'].items():
   data=archive.extractfile(member).read();assert hashlib.sha256(data).hexdigest()==entry['sha256']
   for name,pin in entry['functions'].items():
    body=data[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256']
    if name=='bridge_for_ordinal':
     for ordinal,fn in pins['ordinals'].items():assert len(re.findall(rb'case\s+'+ordinal.encode()+rb':\s*return\s+'+fn.encode()+rb'\s*;',body))==1
    else:text=text.replace('/* PINNED_'+name.upper()+' */',body.decode())
 assert '/* PINNED_' not in text
 modules=[('memory',here.parent/'service-native/memory.rs'),('registers',here.parent/'service-native/registers.rs'),('call_state',here.parent/'service-native/call_state.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs'),('cases',here/'cases.rs'),('probe',here/'probe.rs'),('controls',here/'controls.rs')]
 with tempfile.TemporaryDirectory(prefix='gamepkg-settings-') as temp:
  root=Path(temp);reference=root/'reference.c';reference.write_text(text);vectors=root/'vectors';source_rs=root/'main.rs';rs='#![allow(dead_code)]\n'
  for name,path in [('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),*modules]:rs+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  rs+=(here/'reference.rs').read_text();source_rs.write_text(rs)
  for triple,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(triple+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
   exe=root/(triple+('.exe' if 'windows' in triple else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(source_rs),'-C','link-arg='+str(obj),'-o',str(exe)]
   if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in triple:subprocess.run([str(exe),*([str(vectors)] if triple=='x86_64-unknown-linux-gnu' else [])],check=True,timeout=180)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=180,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+triple+': reference compilation only',flush=True)
  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
   exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
   for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(vectors)],check=True)
 print('NV/service-state component only: no complete title entry, renderer, physical controls, audio or pacing qualification.')
if __name__=='__main__':main()
