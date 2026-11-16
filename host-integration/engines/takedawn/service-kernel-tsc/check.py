#!/usr/bin/env python3
"""Maintainer-only exact pinned TSC comparisons. Actual shared-state admission is in service-kernel-clock/check.py."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,tarfile,tempfile

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--wine',action='store_true');a=p.parse_args();app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']
 with tarfile.open(kit) as archive:data=archive.extractfile(pins['member']).read()
 assert hashlib.sha256(data).hexdigest()==pins['member_sha256']
 values={}
 for name in ['function','constant']:
  entry=pins[name];values[name]=data[entry['start']:entry['end']];assert hashlib.sha256(values[name]).hexdigest()==entry['sha256']
 text=(here/'reference.c.in').read_text().replace('/* PINNED_CONSTANT */',values['constant'].decode())
 # Only rename the identifier with the preprocessor. Every original body byte,
 # including its function-local statics and arithmetic, stays identical.
 text=text.replace('/* PINNED_FAMILIES */','\n'.join('#define xbox_ReadTimeStampCounter original_tsc'+str(i)+'\n'+values['function'].decode()+'\n#undef xbox_ReadTimeStampCounter' for i in range(8)))
 with tempfile.TemporaryDirectory(prefix='gamepkg-tsc-') as temp:
  root=Path(temp);source=root/'reference.c';source.write_text(text)
  rs='#![allow(dead_code)]\n'
  for name,path in [('client',app/'engines/native-services/client.rs'),('memory',here.parent/'service-native/memory.rs'),('registers',here.parent/'service-native/registers.rs'),('call_state',here.parent/'service-native/call_state.rs'),('heap_service',here.parent/'service-heap/service.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs')]:rs+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  main_rs=root/'main.rs';main_rs.write_text(rs+(here/'reference.rs').read_text())
  for target,cc in [('x86_64-unknown-linux-gnu',['gcc']),('i686-unknown-linux-gnu',['gcc-11','-m32']),('x86_64-pc-windows-gnu',['x86_64-w64-mingw32-gcc']),('i686-pc-windows-gnu',['i686-w64-mingw32-gcc'])]:
   obj=root/(target+'.o');subprocess.run(cc+['-std=c11','-O2','-Wall','-Wextra','-Werror','-c',str(source),'-o',str(obj)],check=True)
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(main_rs),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   if target=='i686-pc-windows-gnu':cmd+=['-C','link-arg=-lgcc'] # Maintainer C oracle only, after its object.
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe)],check=True)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=120,env=dict(os.environ,WINEPREFIX='/tmp/gamepkg-port-replace-wine',WINEDEBUG='-all'))
   else:print('PASS '+target+': compilation only')
 print('Pure original-body arithmetic comparison only. This is not shared-state, admitted title, hardware or gameplay qualification.')
if __name__=='__main__':main()
