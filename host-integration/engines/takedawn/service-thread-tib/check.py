#!/usr/bin/env python3
"""Maintainer-only original worker TIB differential and complete source admission."""
import argparse,hashlib,importlib.util,json,os,shutil,subprocess,tarfile,tempfile
from pathlib import Path

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--app',type=Path,required=True);parser.add_argument('--app-lib',type=Path,required=True);parser.add_argument('--wine',action='store_true');parser.add_argument('--evidence',type=Path);a=parser.parse_args()
 app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent;kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';pins=json.loads((here/'reference_inputs.json').read_text());heap_pins=json.loads((here.parent/'service-heap/reference_inputs.json').read_text());assert digest(kit)==pins['source_kit_sha256']==heap_pins['source_kit_sha256']
 def body(data,pin):
  value=data[pin['start']:pin['end']];assert hashlib.sha256(value).hexdigest()==pin['sha256'];return value.decode()
 with tarfile.open(kit) as archive:
  source=archive.extractfile(pins['member']).read();assert hashlib.sha256(source).hexdigest()==pins['source_sha256']
  heap=archive.extractfile(heap_pins['member']).read();assert hashlib.sha256(heap).hexdigest()==heap_pins['source_sha256']
 reference=(here.parent/'service-heap/heap_reference.c.in').read_text()
 for name,pin in heap_pins['functions'].items():reference=reference.replace('/* GAMEPKG_PINNED_'+name.upper()+' */',body(heap,pin))
 reference+='\n'+(here/'reference.c.in').read_text().replace('/* PINNED_THREAD_TIB */',body(source,pins['function']))
 assert '/* PINNED_' not in reference and '/* GAMEPKG_PINNED_' not in reference
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,path in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('layout','../service-memory/layout.rs'),('sha256','../service-image/sha256.rs'),('thread_tib','thread_tib.rs'),('probe','probe.rs')]:modules.append((name,here/path))
 with tempfile.TemporaryDirectory(prefix='gamepkg-thread-tib-') as temp:
  root=Path(temp);c=root/'reference.c';c.write_text(reference);driver=root/'main.rs';driver.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+'mod gamepkg{pub use crate::client::*;}\n'+(here/'reference.rs').read_text());expected=None
  for triple,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(triple+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(c),'-o',str(obj)],check=True)
   exe=root/(triple+('.exe' if 'windows' in triple else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(driver),'-C','link-arg='+str(obj),'-o',str(exe)]
   if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True);vector=root/(triple+'.vectors')
   if 'windows' not in triple:subprocess.run([str(exe),str(vector)],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe),'Z:'+str(vector).replace('/','\\')],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:continue
   data=vector.read_bytes();assert len(data)==364
   if expected is None:expected=data
   else:assert data==expected
  for name,old,new in [
   ('missing-seh','ram.write_u32(tib, u32::MAX)?;','ram.write_u32(tib, 0)?;'),
   ('wrong-slot-zero','ram.write_u32(block, data)?;','ram.write_u32(block, block)?;'),
   ('wrong-stack-word','ram.write_u32(tib + 4, data)?;','ram.write_u32(tib + 4, block)?;'),
   ('uncleared-thread-data','ram.fill(data, 0, 64)?;','ram.fill(data, 0x6d, 64)?;'),
  ]:
   original=(here/'thread_tib.rs').read_text();assert original.count(old)==1
   bad=root/(name+'.rs');bad.write_text(original.replace(old,new));test=root/(name+'-main.rs');test.write_text(driver.read_text().replace(json.dumps(str(here/'thread_tib.rs')),json.dumps(str(bad))))
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(test),'-C','link-arg='+str(root/'x86_64-unknown-linux-gnu.o'),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=120)
   assert result.returncode!=0 and 'original per-thread TIB mode' in result.stderr,(name,result.stderr)
   print('PASS required failing '+name+' control; never admitted',flush=True)
  (root/'vectors').write_bytes(expected)
  spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
  exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
  for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',a.evidence/'vectors.bin');receipts={}
   for path in sorted(root.glob('receipt-*/services.json')):
    record=json.loads(path.read_text());assert len(record['game']['objects'])==len(record['raw_objects'])==8
    name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=digest(path)
   assert len(receipts)==4
   report={'schema':1,'status':'original-thread-TIB-component-not-title-gameplay','original':pins,'sources':{name:digest(path) for name,path in modules},'receipts':receipts,'vectors_sha256':digest(root/'vectors'),'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows':'Wine components' if a.wine else 'compile-only','profile_admission':'blocked'}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('Thread TIB component only; worker bootstrap/context/loop and title closure remain required.')
if __name__=='__main__':main()
