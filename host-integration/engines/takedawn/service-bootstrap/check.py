#!/usr/bin/env python3
"""Maintainer-only exact CPU-startup data checks. No user-side tools or extraction."""
import argparse,hashlib,importlib.util,json,struct,subprocess,tarfile,tempfile
from pathlib import Path

def word(b,at,value):struct.pack_into('<I',b,at,value)
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--app',type=Path,required=True);parser.add_argument('--app-lib',type=Path,required=True);parser.add_argument('--iso',type=Path);args=parser.parse_args();here=Path(__file__).resolve().parent;lib=args.app_lib.resolve(strict=True)
 def module(name,path):
  spec=importlib.util.spec_from_file_location(name,path);result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result
 helpers=module('helpers',here.parent/'service-save/check.py');images=module('images',here.parent/'service-image/check.py');discs=module('discs',here.parent/'service-disc/check.py')
 kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';pins=json.loads((here/'reference_inputs.json').read_text());assert hashlib.sha256(kit.read_bytes()).hexdigest()==pins['source_kit_sha256']
 image_pins=json.loads((here.parent/'service-image/reference_inputs.json').read_text())
 with tarfile.open(kit) as archive:source=archive.extractfile(pins['member']).read();header=archive.extractfile(pins['header_member']).read()
 assert hashlib.sha256(source).hexdigest()==pins['source_sha256']==image_pins['source_sha256'];assert hashlib.sha256(header).hexdigest()==pins['header_sha256']
 with tempfile.TemporaryDirectory(prefix='gamepkg-bootstrap-services-') as temp:
  root=Path(temp);template=(here/'bootstrap_reference.c.in').read_text()
  for name,marker in [('toolkit','/* GAMEPKG_PINNED_TOOLKIT */'),('tib_tls','/* GAMEPKG_PINNED_TIB_TLS */')]:
   pin=pins['regions'][name];body=source[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256'];assert template.count(marker)==1;template=template.replace(marker,body.decode())
  reference=root/'bootstrap_reference.c';reference.write_text(template);obj=root/'bootstrap.o';subprocess.run(['gcc','-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(reference),'-o',str(obj)],check=True)
  pin=image_pins;body=source[pin['section_loop_start']:pin['section_loop_end']];assert hashlib.sha256(body).hexdigest()==pin['section_loop_sha256'];image_reference=root/'image_reference.c';image_reference.write_text((here.parent/'service-image/image_reference.c.in').read_text().replace('/* GAMEPKG_PINNED_SECTION_LOOP */',body.decode()));image_obj=root/'image.o';subprocess.run(['gcc','-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(image_reference),'-o',str(image_obj)],check=True)
  exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern',f'gamepkg={lib}','-L',f'dependency={lib.parent}','-C','link-arg='+str(obj),'-C','link-arg='+str(image_obj),'-o',str(exe)]
  for name in ['cap_std','tempfile','toml']:cmd+=['--extern',f'{name}={helpers.dependency(lib,name)}']
  subprocess.run(cmd,check=True)
  cases=[]
  def add(name,mode,data):
   path=root/(name+'.iso');discs.fixture(path)
   with path.open('r+b') as f:f.truncate(100*2048);f.seek(40*2048+4);f.write(struct.pack('<II',80,len(data)));f.seek(80*2048);f.write(data)
   cases.extend([mode,str(path)])
  def tls(initial,zero):
   data=images.image();word(data,0x12c,0x12000)
   start=0x12100 if initial else 0
   for at,value in [(0xc00,start),(0xc04,start+initial),(0xc08,0x12100),(0xc0c,0),(0xc10,zero),(0xc14,0)]:word(data,at,value)
   return data
  add('no-tls','valid',images.image())
  for initial,zero in [(0,0),(0,144),(1,0),(15,1),(16,0),(17,15),(63,16),(255,0),(512,0),(513,0)]:add(f'tls-{initial}-{zero}','valid',tls(initial,zero))
  for length,mode in [(65520,'valid'),(65521,'invalid')]:
   data=tls(length,0);word(data,0x1b8+8,0x20000);word(data,0x10c,0x40000);add(f'large-tls-{length}',mode,data)
  for name,patches in [('overflow',[(0xc10,0xffffffff)]),('reversed',[(0xc00,0x12100),(0xc04,0x120ff)]),('source-outside-image',[(0xc00,0x500000),(0xc04,0x500010)]),('directory-outside-image',[(0x12c,0x4000)]),('directory-truncated',[(0x12c,0x14fe9)])]:
   data=tls(0,144)
   for at,value in patches:word(data,at,value)
   add(name,'invalid',data)
  for available,mode in [(24*1048576,'valid'),(24*1048576-65536,'valid'),(17*1048576+65536,'valid'),(17*1048576,'valid'),(8*1048576,'valid'),(8*1048576-65536,'invalid'),(2*1048576-65536,'invalid')]:
   high=0x4000000-available-0x80000;data=tls(0,144);word(data,0x1b8+8,high-0x12000);word(data,0x10c,high-0x10000);add(f'stack-available-{available}',mode,data)
  for mode in ['record-revoke','readonly-tib','wrong-view','plan-record-revoke','wrong-code-bounds']:add(mode,mode,tls(0,144))
  data=tls(0,144);word(data,0xc00,0xffffffff);word(data,0xc04,0xffffffff);add('unused-tls-source','valid',data)
  data=tls(0,0);word(data,0x12c,0x14fe8);add('tls-directory-exact-end','valid',data)
  if args.iso:cases.extend(['private',str(args.iso.resolve(strict=True))])
  subprocess.run([str(exe),str(args.app.resolve()),str(here),*cases],check=True)
 print('CPU-data component evidence only; no kernel, device or native gameplay admission.')
if __name__=='__main__':main()
