#!/usr/bin/env python3
"""Maintainer native image-to-import integration; no title entry or extraction."""
import argparse,hashlib,importlib.util,json,struct,subprocess,tempfile
from pathlib import Path

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--call-targets',type=Path,required=True);p.add_argument('--iso',type=Path);p.add_argument('--evidence',type=Path);p.add_argument('--mutations',action='store_true');a=p.parse_args()
 app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 def module(name,path):
  spec=importlib.util.spec_from_file_location(name,path);result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result
 helpers=module('helpers',here.parent/'service-save/check.py');images=module('images',here.parent/'service-image/check.py');discs=module('discs',here.parent/'service-disc/check.py')
 with tempfile.TemporaryDirectory(prefix='gamepkg-image-imports-') as temp:
  root=Path(temp);probe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'image_admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(probe)]
  for name in ['cap_std','tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);cases=[]
  for mode in [0,1,2,3,5,6,7,8,9,10,11,*range(12,23)]:
   data=images.image();images.word(data,0x10c,0x770000);images.word(data,0x1b8+8,0x76e000);images.word(data,0x1b8+16,0x1000)
   images.word(data,0x158,0x12400^(0xefb1f152 if mode==1 else 0x5b6d40b6))
   if mode==1:images.word(data,0x128,0x11020^0x94859d4b)
   data[0xfc0:0x1000+379*4]=bytes(64+379*4)
   for index,ordinal in enumerate([14,181 if mode==2 else 16,23]+([335,336,337,338,339,340,24,125,126,127,128] if mode==10 else [335,336,337,338,339,340,24] if mode==8 or mode==9 else [335,336,337,338,339,340] if mode==7 else [335,336,337] if mode==6 else [])):images.word(data,0x1000+index*4,0x80000000|ordinal)
   if mode>=11:
    for index,ordinal in enumerate([14,16,23,109,98]):images.word(data,0x1000+index*4,0x80000000|ordinal)
   if mode==13:images.word(data,0x1000+4*4,0x800000b5)
   images.word(data,0xffc,0x8000000e)
   image=root/(str(mode)+'.iso');discs.fixture(image)
   with image.open('r+b') as f:f.truncate(100*2048);f.seek(40*2048+4);f.write(struct.pack('<II',80,len(data)));f.seek(80*2048);f.write(data)
   cases.extend([str(mode),str(image)])
  if a.iso:cases.extend(['4',str(a.iso.resolve(strict=True)),'23',str(a.iso.resolve(strict=True))])
  prefix=[str(probe),str(app),str(here),str(a.call_targets.resolve(strict=True))]
  evidence=[]
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);evidence=['--evidence',str(a.evidence.resolve())]
  subprocess.run([*prefix,*evidence,*cases],check=True)
  if a.mutations:
   original=(here/'../service-kernel-context/startup.rs').read_text()
   variants=[
    ('drop-join-barrier','if !self.jobs.is_empty() {','if false {',18),
    ('accept-unrelated-join','if found == self.jobs.len() {','if false {',18),
    ('forget-failed-phase','self.phase = 3;','self.phase = 0;',13),
    ('context-free-planning','image_imports::imports_image_with_context(','image_imports::imports_image_with_context(',12),
    ('drop-prefix-registry-cleanup','self.registry = None;','self.registry = None;',20),
   ]
   for name,old,new,mode in variants:
    if name=='accept-unrelated-join':
     old='if found == self.jobs.len() {\n            return Err(-4);\n        }';new='if found == self.jobs.len() { found = 0; }'
    elif name=='context-free-planning':
     old='image_imports::imports_image_with_context(\n            ram,\n            image,\n            data,\n            remap,\n            self.context.as_ref().ok_or(-3)?,\n        )?';new='image_imports::imports_image(ram, image, data, remap)?'
    elif name=='drop-prefix-registry-cleanup':
     old='let outcome = registry.connections_close(ram);';new='let outcome = Ok(());'
     # Keep an explicit registry use and report completion so quota recovery, not
     # an unused-variable warning or stuck closed flag, detects the leak.
     old='let outcome = registry.connections_close(ram);\n            if !registry.connections_closed() {\n                return outcome;\n            }';new='let _ = registry.connections_id();\n            let outcome: Result<(),i32> = Ok(());'
    assert original.count(old)==1,(name,old)
    variant=root/(name+'.rs');variant.write_text(original.replace(old,new,1))
    result=subprocess.run([*prefix,'--linux64-only','--startup-copy',str(variant),str(mode),str(root/(str(mode)+'.iso'))],capture_output=True,text=True)
    (root/(name+'.log')).write_text(result.stdout+result.stderr)
    assert result.returncode!=0,(name,'broken startup accepted')
    assert 'PASS Linux64: complete image/import/helper source admission and all-object audit before trusted linking' in result.stdout,(name,result.stderr)
    assert 'error:' not in result.stderr and 'BuildError' not in result.stderr,(name,result.stderr)
    assert 'assertion' in result.stderr or 'panicked at' in result.stderr,(name,result.stderr)
    if a.evidence:
     (a.evidence/(name+'.log')).write_text(result.stdout+result.stderr)
     (a.evidence/(name+'.rs')).write_text(variant.read_text())
    print('PASS required failing '+name+' startup control; no policy exemption',flush=True)
  if a.evidence:
   files={str(path.relative_to(a.evidence)):hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(a.evidence.iterdir())}
   sources={str(path.relative_to(here.parent)):hashlib.sha256(path.read_bytes()).hexdigest() for path in [here/'check_image.py',here/'image_admission.rs',here/'image_probe.rs',here/'../service-kernel-context/startup.rs',here/'../service-kernel-context/startup_probe.rs']}
   (a.evidence/'qualification.json').write_text(json.dumps({'schema':1,'sdk':'fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f','status':'authenticated-synthetic-image-startup-and-actual-join-lifetimes-not-title-entry','files':files,'sources':sources,'required_failing_variants':5 if a.mutations else 0,'windows':'source/object/link only','linux':'actual confined x64/i686 children','profile_admission':'blocked'},indent=2)+'\n')
 print('Image/import infrastructure only; private title remains rejected until all actual kernel calls and native entry migrate.')
if __name__=='__main__':main()
