#!/usr/bin/env python3
"""Maintainer-only exact section-loader and strict admission checks; no user tools."""
import argparse,hashlib,importlib.util,json,os,struct,subprocess,tarfile,tempfile
from pathlib import Path

def word(b,at,value):struct.pack_into('<I',b,at,value)
def image():
 b=bytearray(8192);b[:4]=b'XBEH'
 for at,value in [(0x104,0x10000),(0x108,1024),(0x10c,65536),(0x11c,2),(0x120,0x10180),(0x128,0x11020^0xa8fc57ab),(0x158,0x12000^0x5b6d40b6)]:word(b,at,value)
 for at,flags,va,vs,raw,rs,name in [(0x180,4,0x11000,0x1000,0x400,0x800,0x10200),(0x1b8,3,0x12000,0x3000,0xc00,0x400,0x10208)]:
  for shift,value in [(0,flags),(4,va),(8,vs),(12,raw),(16,rs),(20,name)]:word(b,at+shift,value)
 b[0x200:0x206]=b'.text\0';b[0x208:0x20e]=b'.data\0'
 for n in range(0x400,len(b)):b[n]=(n*37)&255
 return b

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--app',type=Path,required=True);parser.add_argument('--app-lib',type=Path,required=True);parser.add_argument('--iso',type=Path);parser.add_argument('--wine',action='store_true');args=parser.parse_args()
 here=Path(__file__).resolve().parent;lib=args.app_lib.resolve(strict=True)
 spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
 spec=importlib.util.spec_from_file_location('disc_checks',here.parent/'service-disc/check.py');discs=importlib.util.module_from_spec(spec);spec.loader.exec_module(discs)
 pin=json.loads((here/'reference_inputs.json').read_text())
 assert set(pin['reused_sources'])=={'../service-disc/disc.rs','../service-native/memory.rs','../service-native/call_state.rs'}
 for name,expected in pin['reused_sources'].items():assert hashlib.sha256((here/name).read_bytes()).hexdigest()==expected
 kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert hashlib.sha256(kit.read_bytes()).hexdigest()==pin['source_kit_sha256']
 with tarfile.open(kit) as archive:original=archive.extractfile(pin['member']).read()
 assert hashlib.sha256(original).hexdigest()==pin['source_sha256'];body=original[pin['section_loop_start']:pin['section_loop_end']];assert hashlib.sha256(body).hexdigest()==pin['section_loop_sha256']
 with tempfile.TemporaryDirectory(prefix='gamepkg-image-services-') as temp:
  root=Path(temp);reference=root/'reference.c';template=(here/'image_reference.c.in').read_text();assert template.count('/* GAMEPKG_PINNED_SECTION_LOOP */')==1;reference.write_text(template.replace('/* GAMEPKG_PINNED_SECTION_LOOP */',body.decode()))
  obj=root/'reference.o';subprocess.run(['gcc','-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
  exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern',f'gamepkg={lib}','-L',f'dependency={lib.parent}','-C','link-arg='+str(obj),'-o',str(exe)]
  for name in ['cap_std','tempfile','toml']:cmd+=['--extern',f'{name}={helpers.dependency(lib,name)}']
  subprocess.run(cmd,check=True)
  sha=root/'sha-reference';vectors=root/'sha-vectors';subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(here/'sha_reference.rs'),'--extern',f'gamepkg={lib}','-L',f'dependency={lib.parent}','-o',str(sha)],check=True);subprocess.run([str(sha),str(vectors)],check=True)
  for triple in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   binary=root/('sha-'+triple+('.exe' if 'windows' in triple else ''));command=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(here/'sha_vectors.rs'),'-o',str(binary)]
   if triple=='i686-unknown-linux-gnu':command+=['-C','linker=/usr/bin/gcc-11']
   subprocess.run(command,check=True)
   if 'windows' not in triple:subprocess.run([str(binary),str(vectors)],check=True)
   elif args.wine:subprocess.run(['wine',str(binary),'Z:'+str(vectors).replace('/','\\')],check=True,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'),timeout=90)
   else:print('Compile-only SHA replay:',triple)
  cases=[]
  def add(name,mode,data):
   path=root/(name+'.iso');discs.fixture(path)
   with path.open('r+b') as f:
    f.truncate(100*2048);f.seek(40*2048+4);f.write(struct.pack('<II',80,len(data)));f.seek(80*2048);f.write(data)
   cases.extend([mode,str(path)])
  add('normal','valid',image())
  for name,patches in [('raw-larger',[(0x180+16,0x1800)]),('virtual-overlap',[(0x1b8+4,0x11700),(0x158,0x11700^0x5b6d40b6)]),('header-overlap',[(0x180+4,0x10000),(0x128,0x10020^0xa8fc57ab)]),('data-bss',[(0x1b8+16,0),(0x1b8+12,0xffffffff)]),('both-executable',[(0x1b8,7)]),('debug-keys',[(0x128,0x11020^0x94859d4b),(0x158,0x12000^0xefb1f152)])]:
   data=image()
   for at,value in patches:word(data,at,value)
   add(name,'valid',data)
  for name,patches in [('zero-headers',[(0x108,0)]),('large-headers',[(0x108,65537)]),('zero-sections',[(0x11c,0)]),('too-many-sections',[(0x11c,65)]),('bad-table',[(0x120,0xfffffff0)]),('bad-size',[(0x10c,0xffffffff)]),('low-section',[(0x180+4,0)]),('high-section',[(0x180+8,0x4000000)]),('zero-section',[(0x180+8,0)]),('truncated-copy',[(0x180+12,0xffffff00)]),('bad-name',[(0x180+20,0xffffffff)]),('bss-entry',[(0x128,0x11900^0xa8fc57ab)]),('bad-thunk',[(0x158,0xffffffff)])]:
   data=image()
   for at,value in patches:word(data,at,value)
   add(name,'invalid',data)
  for name,patches in [('short-image',[(0x10c,512)]),('low-table',[(0x120,0xffff)]),('table-outside-header',[(0x120,0x103fc)]),('no-executable',[(0x180,0)]),('thunk-at-end',[(0x158,0x4000000^0x5b6d40b6)])]:
   data=image()
   for at,value in patches:word(data,at,value)
   add(name,'invalid',data)
  data=image();data[:4]=b'NOPE';add('bad-magic','invalid',data)
  data=image();word(data,0x180+20,0x10300);data[0x300:0x400]=b'\xff'*256;add('unterminated-name','invalid',data)
  add('revoked-before-load','revoke',image());add('wrong-hash','hash-mismatch',image())
  if args.iso:cases.extend(['private',str(args.iso.resolve(strict=True))])
  subprocess.run([str(exe),str(args.app.resolve()),str(here),*cases],check=True)
 print('Component evidence only; no complete native bootstrap or playable title.')
if __name__=='__main__':main()
