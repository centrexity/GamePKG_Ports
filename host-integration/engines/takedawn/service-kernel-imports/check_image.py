#!/usr/bin/env python3
"""Maintainer native image-to-import integration; no title entry or extraction."""
import argparse,importlib.util,struct,subprocess,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--call-targets',type=Path,required=True);p.add_argument('--iso',type=Path);a=p.parse_args()
 app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 def module(name,path):
  spec=importlib.util.spec_from_file_location(name,path);result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result
 helpers=module('helpers',here.parent/'service-save/check.py');images=module('images',here.parent/'service-image/check.py');discs=module('discs',here.parent/'service-disc/check.py')
 with tempfile.TemporaryDirectory(prefix='gamepkg-image-imports-') as temp:
  root=Path(temp);probe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'image_admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(probe)]
  for name in ['cap_std','tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);cases=[]
  for mode in [0,1,2,3,5,6,7,8,9,10,11]:
   data=images.image();images.word(data,0x10c,0x770000);images.word(data,0x1b8+8,0x76e000);images.word(data,0x1b8+16,0x1000)
   images.word(data,0x158,0x12400^(0xefb1f152 if mode==1 else 0x5b6d40b6))
   if mode==1:images.word(data,0x128,0x11020^0x94859d4b)
   data[0xfc0:0x1000+379*4]=bytes(64+379*4)
   for index,ordinal in enumerate([14,181 if mode==2 else 16,23]+([335,336,337,338,339,340,24,125,126,127,128] if mode==10 else [335,336,337,338,339,340,24] if mode==8 or mode==9 else [335,336,337,338,339,340] if mode==7 else [335,336,337] if mode==6 else [])):images.word(data,0x1000+index*4,0x80000000|ordinal)
   if mode==11:
    for index,ordinal in enumerate([14,16,23,109,98]):images.word(data,0x1000+index*4,0x80000000|ordinal)
   images.word(data,0xffc,0x8000000e)
   image=root/(str(mode)+'.iso');discs.fixture(image)
   with image.open('r+b') as f:f.truncate(100*2048);f.seek(40*2048+4);f.write(struct.pack('<II',80,len(data)));f.seek(80*2048);f.write(data)
   cases.extend([str(mode),str(image)])
  if a.iso:cases.extend(['4',str(a.iso.resolve(strict=True))])
  subprocess.run([str(probe),str(app),str(here),str(a.call_targets.resolve(strict=True)),*cases],check=True)
 print('Image/import infrastructure only; private title remains rejected until all actual kernel calls and native entry migrate.')
if __name__=='__main__':main()
