#!/usr/bin/env python3
"""Maintainer-only recursive gate service checks, not IRQ/IRQL qualification."""
import argparse,importlib.util,json,os,subprocess,tempfile
from pathlib import Path
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');a=p.parse_args();app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent
 mods=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('registers',here.parent/'service-native/registers.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs'),('gate',here/'gate.rs'),('probe',here/'probe.rs')]
 with tempfile.TemporaryDirectory(prefix='gamepkg-raw-gate-') as temp:
  root=Path(temp);source=root/'runtime.rs';source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n'for name,path in mods)+(here/'runtime.rs').read_text())
  for target in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe)],check=True,timeout=60)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=60,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h);exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
   for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(h.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here)],check=True)
 print('PASS: actual recursive lock, shared counts, reviewed-task contention and joined cleanup; not IRQ safe points, IRQL, callback delivery or gameplay.')
if __name__=='__main__':main()
