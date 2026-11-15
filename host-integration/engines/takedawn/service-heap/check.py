#!/usr/bin/env python3
"""Maintainer-only exact original heap comparisons and confined admission checks."""
import argparse,hashlib,importlib.util,json,os,subprocess,tarfile,tempfile
from pathlib import Path

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--app',type=Path,required=True);parser.add_argument('--app-lib',type=Path,required=True);parser.add_argument('--wine',action='store_true');args=parser.parse_args();app=args.app.resolve(strict=True);lib=args.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
 pin=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert hashlib.sha256(kit.read_bytes()).hexdigest()==pin['source_kit_sha256']
 with tarfile.open(kit) as archive:source=archive.extractfile(pin['member']).read()
 assert hashlib.sha256(source).hexdigest()==pin['source_sha256']
 with tempfile.TemporaryDirectory(prefix='gamepkg-heap-services-') as temp:
  scratch=Path(temp);text=(here/'heap_reference.c.in').read_text()
  for name,pinned in pin['functions'].items():
   marker='/* GAMEPKG_PINNED_'+name.upper()+' */';assert text.count(marker)==1;body=source[pinned['start']:pinned['end']];assert hashlib.sha256(body).hexdigest()==pinned['sha256'];text=text.replace(marker,body.decode())
  reference=scratch/'reference.c';reference.write_text(text)
  wrapper='#![allow(dead_code)]\n'
  for name,path in [('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('heap',here/'heap.rs'),('heap_service',here/'service.rs'),('denials',here/'denials.rs'),('reference_cases',here/'reference_cases.rs')]:wrapper+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  wrapper+='#[allow(unused_imports)]mod gamepkg{pub use crate::client::*;}\n'+(here/'reference_main.rs').read_text();main_rs=scratch/'main.rs';main_rs.write_text(wrapper)
  native_obj=None
  for triple,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=scratch/(triple+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
   exe=scratch/(triple+('.exe' if 'windows' in triple else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(main_rs),'-C','link-arg='+str(obj),'-o',str(exe)]
   if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11']
   subprocess.run(cmd,check=True)
   if 'windows' not in triple:subprocess.run([str(exe)],check=True,timeout=180)
   elif args.wine:subprocess.run(['wine',str(exe)],check=True,timeout=180,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+triple+': reference compilation only',flush=True)
   if triple=='x86_64-unknown-linux-gnu':native_obj=obj
  host=scratch/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-C','link-arg='+str(native_obj),'-o',str(host)]
  for name in ['cap_std','tempfile','toml']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(host),str(app),str(here)],check=True)
 print('Heap bookkeeping/data component only; no kernel binding, shared task integration or native gameplay admission.')
if __name__=='__main__':main()
