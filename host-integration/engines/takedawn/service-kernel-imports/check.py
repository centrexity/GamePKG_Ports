#!/usr/bin/env python3
"""Exact original import preparation versus dependency-free checked Rust."""
import argparse,hashlib,importlib.util,json,os,re,subprocess,tarfile,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--call-targets',type=Path,required=True);p.add_argument('--wine',action='store_true');a=p.parse_args()
 app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);targets=a.call_targets.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'reference_inputs.json').read_text());data_pins=json.loads((here.parent/'service-kernel-data/reference_inputs.json').read_text());heap_pins=json.loads((here.parent/'service-heap/reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
 assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']==data_pins['source_kit_sha256']==heap_pins['source_kit_sha256']
 def body(data,pin):
  result=data[pin['start']:pin['end']];assert hashlib.sha256(result).hexdigest()==pin['sha256'];return result.decode()
 with tarfile.open(kit) as archive:
  heap=archive.extractfile(heap_pins['member']).read();assert hashlib.sha256(heap).hexdigest()==heap_pins['source_sha256'];text=(here.parent/'service-heap/heap_reference.c.in').read_text()
  for name,pin in heap_pins['functions'].items():text=text.replace('/* GAMEPKG_PINNED_'+name.upper()+' */',body(heap,pin))
  data=(here.parent/'service-kernel-data/reference.c.in').read_text()
  for entry in data_pins['sources'].values():
   source=archive.extractfile(entry['member']).read();assert hashlib.sha256(source).hexdigest()==entry['sha256']
   for name,pin in entry['functions'].items():data=data.replace('/* PINNED_'+name.upper()+' */',body(source,pin))
  header=archive.extractfile(data_pins['header']['member']).read();assert hashlib.sha256(header).hexdigest()==data_pins['header']['sha256'];data=data.replace('/* PINNED_KDATA_DEFINITIONS */',b'\n'.join(re.findall(rb'(?m)^#define KDATA_[^\n]*',header)).decode())
  source=archive.extractfile(pins['member']).read();assert hashlib.sha256(source).hexdigest()==pins['source_sha256'];original_lookup=body(source,pins['functions']['bridge_for_ordinal'])
  for ordinal,name in {'14':'ExAllocatePool','15':'ExAllocatePoolWithTag','17':'ExFreePool','23':'ExQueryPoolBlockSize','165':'MmAllocateContiguousMemory','166':'MmAllocateContiguousMemoryEx','168':'MmClaimGpuInstanceMemory','171':'MmFreeContiguousMemory','180':'MmQueryAllocationSize'}.items():assert len(re.findall(r'case\s+'+ordinal+r':\s*return\s+bridge_'+name+r'\s*;',original_lookup))==1
  wrapper=(here/'reference.c.in').read_text()
  for name,pin in pins['functions'].items():
   if name!='bridge_for_ordinal':wrapper=wrapper.replace('/* PINNED_'+name.upper()+' */',body(source,pin))
  memory=archive.extractfile(pins['scan']['member']).read();assert hashlib.sha256(memory).hexdigest()==pins['scan']['source_sha256'];wrapper=wrapper.replace('/* PINNED_ORIGINAL_THUNK_SCAN */',body(memory,pins['scan']))
  header=archive.extractfile(pins['header']['member']).read();assert hashlib.sha256(header).hexdigest()==pins['header']['sha256'];definitions=[]
  for name in ['XBOX_KERNEL_THUNK_TABLE_BASE','XBOX_KERNEL_THUNK_TABLE_SIZE']:
   definitions.append(re.search(rb'(?m)^#define\s+'+name.encode()+rb'[^\n]*',header)[0])
  wrapper=wrapper.replace('/* PINNED_THUNK_DEFINITIONS */',b'\n'.join(definitions).decode())
  assert '/* PINNED_' not in data+wrapper and 'GAMEPKG_PINNED_' not in text
 modules=[('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('layout',here.parent/'service-memory/layout.rs'),('kernel_data',here.parent/'service-kernel-data/kernel_data.rs'),('signature',here.parent/'service-kernel-data/signature.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs')]
 modules += [(name,here.parent/'service-native'/f'{name}.rs') for name in ['registers','calls_runtime','vector','scalar','debug']]
 modules += [('game',here.parent/'service-native/generated/game.rs'),('dispatch',here.parent/'service-native/generated/dispatch.rs')]
 modules += [(name,here/f'{name}.rs') for name in ['imports','cases','probe','controls']]
 with tempfile.TemporaryDirectory(prefix='gamepkg-imports-') as temp:
  root=Path(temp);reference=root/'reference.c';reference.write_text(text+'\n'+data+'\n'+wrapper);vectors=root/'vectors';source_rs=root/'main.rs';rs='#![allow(non_camel_case_types,non_snake_case,non_upper_case_globals,unused_assignments,unused_variables,unused_mut,unused_parens,dead_code,unreachable_code)]\n'
  for name,path in [('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),*modules]:rs+='#[path='+json.dumps(str(path))+']mod '+name+';\n'
  report=json.loads((targets/'call_targets.json').read_text());assert report['production_ready'] is False
  for name,digest in report['emitted_files'].items():
   assert name.endswith('.rs') and '..' not in Path(name).parts and not Path(name).is_absolute();payload=(targets/name).read_bytes();assert hashlib.sha256(payload).hexdigest()==digest;dest=root/name;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(payload)
  rs+='mod targets;\n#[allow(unused_imports)]mod gamepkg{pub use crate::client::*;}\n'+(here/'reference.rs').read_text();source_rs.write_text(rs)
  for triple,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(triple+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
   exe=root/(triple+('.exe' if 'windows' in triple else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(source_rs),'-C','link-arg='+str(obj),'-o',str(exe)]
   if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in triple:subprocess.run([str(exe),*([str(vectors)] if triple=='x86_64-unknown-linux-gnu' else [])],check=True,timeout=180)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=180,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+triple+': reference compilation only',flush=True)
  spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
  exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
  for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(vectors),str(targets)],check=True)
 print('Import component only: all other kernel calls and complete title entry remain gated.')
if __name__=='__main__':main()
