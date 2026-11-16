#!/usr/bin/env python3
"""Maintainer exact original dispatcher initialization and strict admission checks."""
import argparse,hashlib,importlib.util,json,os,re,subprocess,tarfile,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');a=p.parse_args()
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent;pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256'];text=(here/'reference.c.in').read_text()
 with tarfile.open(kit) as t:data=t.extractfile(pins['member']).read();header=t.extractfile(pins['header']['member']).read()
 assert hashlib.sha256(data).hexdigest()==pins['source_sha256'] and hashlib.sha256(header).hexdigest()==pins['header']['sha256']
 for name,pin in pins['functions'].items():
  body=data[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256']
  if name=='bridge_for_ordinal':
   for ordinal,(fn,_) in pins['ordinals'].items():assert len(re.findall(rb'case\s+'+ordinal.encode()+rb':\s*return\s+'+fn.encode()+rb'\s*;',body))==1
  elif name=='stdcall_args_for_ordinal':
   for ordinal,(_,count) in pins['ordinals'].items():assert len(re.findall(rb'case\s+'+ordinal.encode()+rb':\s*return\s+'+str(count).encode()+rb'\s*;',body))==1
  else:text=text.replace('/* PINNED_'+name.upper()+' */',body.decode())
 for name,pin in pins['header']['definitions'].items():
  body=header[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256'];text=text.replace('/* PINNED_'+name+' */',body.decode())
 assert '/* PINNED_' not in text
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('registers',here.parent/'service-native/registers.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs'),('sha256',here.parent/'service-image/sha256.rs'),('cases',here/'cases.rs'),('probe',here/'probe.rs'),('controls',here/'controls.rs'),('kernel_data',here.parent/'service-kernel-data/kernel_data.rs'),('signature',here.parent/'service-kernel-data/signature.rs'),('tick_service',here.parent/'service-kernel-tick/tick.rs')]
 with tempfile.TemporaryDirectory(prefix='gamepkg-dispatcher-') as temp:
  root=Path(temp);reference=root/'reference.c';reference.write_text(text);vectors=root/'vectors';rs='#![allow(dead_code)]\n'
  for name,path in modules:rs+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  source=root/'main.rs';source.write_text(rs+(here/'reference.rs').read_text())
  for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe),*([str(vectors)] if target.startswith('x86_64') else [])],check=True,timeout=180)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=180,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+target+': compilation only')
  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
   exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
   for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(vectors)],check=True)
 print('Dispatcher object initialization only: not IRQ delivery, DPC execution, timers, complete title or physical gameplay qualification.')
if __name__=='__main__':main()
