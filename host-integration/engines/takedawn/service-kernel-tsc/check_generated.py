#!/usr/bin/env python3
"""Maintainer pinned generated TSC leaves: original-body comparisons and real source/object admission."""
import argparse,hashlib,importlib.util,json,os,subprocess,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--units',type=Path,required=True);p.add_argument('--tool',type=Path,required=True);p.add_argument('--call-targets',type=Path,required=True);p.add_argument('--wine',action='store_true');a=p.parse_args()
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent;lib=a.app_lib.resolve(strict=True);tool=a.tool.resolve(strict=True)
 with tempfile.TemporaryDirectory(prefix='gamepkg-tsc-leaves-') as temp:
  root=Path(temp);prepared=root/'prepared';subprocess.run([str(tool),'--prepare-tsc-leaves',str(a.units.resolve(strict=True)),str(prepared)],check=True)
  pin=json.loads((here/'generated.json').read_text());proof=json.loads((prepared/'provenance.json').read_text());assert proof==pin and proof['production_ready'] is False
  assert (here/'generated.rs').read_bytes()==(prepared/'game.rs').read_bytes();assert hashlib.sha256((prepared/'game.rs').read_bytes()).hexdigest()==proof['game_sha256']
  oracle=(prepared/'oracle.rs').read_text();assert hashlib.sha256(oracle.encode()).hexdigest()==proof['oracle_sha256']
  rs='#![allow(dead_code,non_camel_case_types,non_snake_case,non_upper_case_globals,unused_mut,unused_parens)]\n'
  for name,path in [('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('registers',here.parent/'service-native/registers.rs'),('call_state',here.parent/'service-native/call_state.rs'),('heap_service',here.parent/'service-heap/service.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('real_kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs'),('leaf_probe',here/'leaf_probe.rs'),('generated',here/'generated.rs'),('calls_runtime',here.parent/'service-native/calls_runtime.rs'),('dispatch',here/'leaf_dispatch.rs')]:rs+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  target_root=a.call_targets.resolve(strict=True);target_proof=json.loads((target_root/'call_targets.json').read_text())
  for name,expected in target_proof['emitted_files'].items():
   assert name.endswith('.rs') and '..' not in Path(name).parts
   data=(target_root/name).read_bytes();assert hashlib.sha256(data).hexdigest()==expected
   destination=root/name;destination.parent.mkdir(parents=True,exist_ok=True);destination.write_bytes(data)
  rs+='mod targets;\n'
  source=root/'main.rs';source.write_text(rs+(here/'leaf_reference.rs').read_text().replace('/* PINNED_ORACLE */',oracle))
  for target in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe)],check=True)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=180,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+target+': compilation only')
  spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
  admission=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'leaf_admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(admission)]
  for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(admission),str(app),str(here),str(prepared/'oracle.rs'),str(a.call_targets.resolve(strict=True))],check=True)
 print('Five timestamp leaf bodies only; not complete native title/renderer/production launcher/gameplay qualification.')
if __name__=='__main__':main()
