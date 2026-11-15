#!/usr/bin/env python3
"""Maintainer-only checked heap task/client concurrency and protocol qualification."""
import argparse,importlib.util,json,os,subprocess,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--wine',action='store_true');a=p.parse_args();app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
 with tempfile.TemporaryDirectory(prefix='gamepkg-shared-heap-') as temp:
  scratch=Path(temp);text='#![allow(dead_code)]\n'
  for name,path in [('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('heap',here/'heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here/'service.rs'),('service_checks',here/'service_checks.rs')]:text+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  text+='#[allow(unused_imports)]mod gamepkg{pub use crate::client::*;}\n'+(here/'service_reference.rs').read_text();source=scratch/'main.rs';source.write_text(text)
  for triple in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   exe=scratch/(triple+('.exe' if 'windows' in triple else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(source),'-o',str(exe)]
   if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11']
   subprocess.run(cmd,check=True)
   if 'windows' not in triple:subprocess.run([str(exe)],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+triple+': runtime compilation only',flush=True)
  exe=scratch/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'service_admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
  for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here)],check=True)
 print('Shared heap task component only; kernel exports, title-thread startup and actual gameplay remain unqualified.')
if __name__=='__main__':main()
