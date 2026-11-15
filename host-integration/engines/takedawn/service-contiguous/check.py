#!/usr/bin/env python3
"""Maintainer-only exact original contiguous allocator and checked-source tests."""
import argparse,hashlib,importlib.util,json,os,re,subprocess,tarfile,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');a=p.parse_args();app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent
 pin=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert hashlib.sha256(kit.read_bytes()).hexdigest()==pin['source_kit_sha256']
 with tarfile.open(kit) as archive:source=archive.extractfile(pin['member']).read();header=archive.extractfile(pin['header']).read()
 assert hashlib.sha256(source).hexdigest()==pin['source_sha256'];assert hashlib.sha256(header).hexdigest()==pin['header_sha256']
 for name,value in pin['defines'].items():assert re.search(rb'^#define\s+'+name.encode()+rb'\s+([^\n]+)',header,re.M).group(1).decode().strip()==value
 for data,defines in [(header,pin['defines']),(source,pin['source_defines'])]:
  template=(here/'contiguous_reference.c.in').read_bytes()
  for name,value in defines.items():
   for current in [data,template]:
    actual=re.search(rb'^#define\s+'+name.encode()+rb'\s+([^\n]+)',current,re.M).group(1).decode().strip()
    assert re.sub(r'\s+','',actual)==re.sub(r'\s+','',value)
 for pattern in [rb'g_contig_used\[CONTIG_PAGES\]\s*=\s*\{\s*CONTIG_RESERVED\s*\}',rb'g_contig_high\s*=\s*CONTIG_PAGE',rb'g_contig_first_free\s*=\s*1']:
  assert len(re.findall(pattern,source))==1
 with tempfile.TemporaryDirectory(prefix='gamepkg-contiguous-') as temp:
  scratch=Path(temp);text=(here/'contiguous_reference.c.in').read_text()
  for name,body in pin['functions'].items():
   value=source[body['start']:body['end']];assert hashlib.sha256(value).hexdigest()==body['sha256'];marker='/* GAMEPKG_PINNED_'+name.upper()+' */';assert text.count(marker)==1;text=text.replace(marker,value.decode())
  reference=scratch/'reference.c';reference.write_text(text)
  wrapper='#![allow(dead_code)]\n'
  for name,path in [('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('heap',here.parent/'service-heap/heap.rs'),('heap_service',here.parent/'service-heap/service.rs'),('contiguous',here/'contiguous.rs'),('denials',here/'denials.rs'),('reference_cases',here/'reference_cases.rs')]:wrapper+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  wrapper+='#[allow(unused_imports)]mod gamepkg{pub use crate::client::*;}\n'+(here/'reference_main.rs').read_text();rs=scratch/'main.rs';rs.write_text(wrapper)
  native_obj=None
  for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=scratch/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
   exe=scratch/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(rs),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe)],check=True,timeout=240)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=240,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+target+': reference compilation only',flush=True)
   if target=='x86_64-unknown-linux-gnu':native_obj=obj
  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
   exe=scratch/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-C','link-arg='+str(native_obj),'-o',str(exe)]
   for name in ['cap_std','tempfile','toml']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here)],check=True)
 print('Contiguous data component only: shared-owner/kernel/device/title integration and gameplay remain required.')
if __name__=='__main__':main()
