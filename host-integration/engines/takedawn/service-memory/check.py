#!/usr/bin/env python3
"""Maintainer-only topology pins, actual shared-RAM conformance and admission."""
import argparse,hashlib,importlib.util,json,os,subprocess,tarfile,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--wine',action='store_true');a=p.parse_args()
 app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
 assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']
 with tarfile.open(kit) as archive:source=archive.extractfile(pins['member']).read();header=archive.extractfile(pins['header_member']).read()
 assert hashlib.sha256(source).hexdigest()==pins['source_sha256'];assert hashlib.sha256(header).hexdigest()==pins['header_sha256']
 for pin in pins['regions'].values():assert hashlib.sha256(source[pin['start']:pin['end']]).hexdigest()==pin['sha256']
 print('PASS: exact reviewed source/header and original mirror/tiled/flash region pins; no OS-mapping oracle claim',flush=True)
 modules=[('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('layout',here/'layout.rs'),('layout_checks',here/'layout_checks.rs')]
 with tempfile.TemporaryDirectory(prefix='gamepkg-layout-') as temp:
  scratch=Path(temp);wrapper='#![allow(dead_code)]\n'
  for name,path in [('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),*modules]:wrapper+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  wrapper+='#[allow(unused_imports)]mod gamepkg{pub use crate::client::*;}\n'+(here/'reference.rs').read_text();source_rs=scratch/'main.rs';source_rs.write_text(wrapper)
  for triple in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   exe=scratch/(triple+('.exe' if 'windows' in triple else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(source_rs),'-o',str(exe)]
   if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11']
   subprocess.run(cmd,check=True)
   if 'windows' not in triple:subprocess.run([str(exe)],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+triple+': compilation only',flush=True)
  spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
  exe=scratch/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
  for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here)],check=True)
 print('Guest-layout component only; no MMIO model, full title boot, physical input, audio or graphics/frame-pacing qualification.')
if __name__=='__main__':main()
