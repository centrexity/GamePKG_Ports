#!/usr/bin/env python3
"""Maintainer authenticated image -> KIC3 object-owned startup; no title entry."""
import argparse,hashlib,importlib.util,json,struct,subprocess,tempfile
from pathlib import Path
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--call-targets',type=Path,required=True);p.add_argument('--iso',type=Path);p.add_argument('--evidence',type=Path);p.add_argument('--mutations',action='store_true');a=p.parse_args();app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 def module(name,path):
  spec=importlib.util.spec_from_file_location(name,path);result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result
 helpers=module('helpers',here.parent/'service-save/check.py');images=module('images',here.parent/'service-image/check.py');discs=module('discs',here.parent/'service-disc/check.py')
 with tempfile.TemporaryDirectory(prefix='gamepkg-object-startup-') as temp:
  root=Path(temp);probe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'image_admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(probe)]
  for name in ['cap_std','tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);cases=[]
  for mode in [*range(51,67),68]:
   data=images.image();images.word(data,0x10c,0x770000);images.word(data,0x1b8+8,0x76e000);images.word(data,0x1b8+16,0x1000);images.word(data,0x158,0x12400^0x5b6d40b6)
   data[0xfc0:0x1000+379*4]=bytes(64+379*4)
   for index,ordinal in enumerate([14,16,23,109,98,113]+([189,225] if mode==68 else [])):images.word(data,0x1000+index*4,0x80000000|ordinal)
   images.word(data,0xffc,0x8000000e)
   image=root/(str(mode)+'.iso');discs.fixture(image)
   with image.open('r+b') as f:f.truncate(100*2048);f.seek(40*2048+4);f.write(struct.pack('<II',80,len(data)));f.seek(80*2048);f.write(data)
   cases.extend([str(mode),str(image)])
  if a.iso:cases.extend(['67',str(a.iso.resolve(strict=True))])
  prefix=[str(probe),str(app),str(here),str(a.call_targets.resolve(strict=True)),'--objects-layout'];evidence=[]
  if a.evidence:a.evidence.mkdir(parents=True,exist_ok=False);evidence=['--evidence',str(a.evidence.resolve())]
  subprocess.run([*prefix,*evidence,*cases],check=True)
  variants=[]
  if a.mutations:
   original=(here/'../service-kernel-context/startup.rs').read_text()
   import re
   old=re.search(r'kernel_pool::KernelContext::kernel_context_new_with_objects\([\s\S]*?self\.objects\.as_ref\(\)\.ok_or\(-3\)\?\.events_id\(\),\s*\)\?',original).group(0)
   variants.append(('omit-object-context',old,'kernel_pool::KernelContext::kernel_context_new_with_shadow(ram,self.registry.as_ref().ok_or(-3)?,self.shadow.as_ref().ok_or(-3)?)?',51))
   start=original.index('        if let Some(domain) = self.objects.as_mut() {');end=original.index('        if let Some(map) = self.handles.as_mut()',start)
   variants.append(('skip-native-domain-drain',original[start:end],'        if self.objects.is_some(){self.objects=None;}\n',51))
   start=original.index('        if let Some(map) = self.handles.as_mut() {');end=original.index('        if let Some(map) = self.shadow.as_mut()',start)
   variants.append(('skip-tagged-backing-cleanup',original[start:end],'        if self.handles.is_some(){self.handles=None;}\n',51))
   old='if ids\n                != [\n                    owned.handles_id(),\n                    self.objects.as_ref().ok_or(-4)?.events_id(),\n                ]'
   variants.append(('allow-substitute-domain',old,old+' && false',64))
   variants.append(('forget-object-job-join','if !self.jobs.is_empty() {','if false {',51))
   start=original.index('            self.handles = Some(handles::Handles::handles_new(ram)?);');end=original.index('            self.objects = Some(',start)
   variants.append(('discard-failed-bind-prefix',original[start:end],'''            let mut staged=handles::Handles::handles_new(ram)?;
            staged.handles_bind(ram,objects[0],objects[1])?;
            self.handles=Some(staged);
''',54))
   old='if !domain.events_closed() {\n                return outcome;\n            }'
   variants.append(('forget-incomplete-domain-close',old,'if !domain.events_closed(){self.close_error=0;}',62))
   for name,old,new,mode in variants:
    assert original.count(old)==1,(name,old);variant=root/(name+'.rs');variant.write_text(original.replace(old,new,1))
    result=subprocess.run([*prefix,'--linux64-only','--startup-copy',str(variant),str(mode),str(root/(str(mode)+'.iso'))],capture_output=True,text=True)
    log=result.stdout+result.stderr
    assert result.returncode!=0,(name,'broken startup accepted')
    assert 'PASS Linux64: complete image/import/helper source admission and all-object audit before trusted linking' in result.stdout,(name,result.stderr)
    assert 'error:' not in result.stderr and 'BuildError' not in result.stderr,(name,result.stderr)
    assert 'assertion' in result.stderr or 'panicked at' in result.stderr,(name,result.stderr)
    if a.evidence:(a.evidence/(name+'.log')).write_text(log);(a.evidence/(name+'.rs')).write_text(variant.read_text())
    print('PASS required failing '+name+' authenticated object-startup control; no policy exemption',flush=True)
  if a.evidence:
   files={p.name:digest(p) for p in a.evidence.iterdir() if p.is_file()};sources={}
   for path in [here/'check_objects.py',here/'image_admission.rs',here/'image_probe.rs',here/'probe.rs',here/'../service-kernel-context/startup.rs',here/'../service-kernel-context/object_startup_probe.rs',here/'../service-kernel-pool/kernel_pool.rs',here/'../service-memory/layout.rs',here/'../service-kernel-handles/handles.rs',here/'../service-kernel-events/events.rs',here/'../service-kernel-event-imports/probe.rs']:sources[str(path.relative_to(here.parent))]=digest(path)
   receipts=[json.loads(path.read_text()) for path in sorted(a.evidence.glob('receipt-*.json'))];assert len(receipts)==4
   sdk_ids={r['game']['sdk'] for r in receipts};assert len(sdk_ids)==1
   (a.evidence/'qualification.json').write_text(json.dumps({'schema':1,'sdk':sdk_ids.pop(),'status':'authenticated-KIC3-owned-event-imports-and-startup-not-title-entry','files':files,'sources':sources,'required_failing_variants':len(variants),'synthetic_modes':[*range(51,67),68],'private_mode':67 if a.iso else None,'linux':'actual confined x64/i686 children','windows':'source/object/link only','profile_admission':'blocked','main_import_gap':'100 with KIC3, 102 with KIC2'},indent=2)+'\n')
 print('Authenticated startup routes NtCreateEvent/NtSetEvent; other kernel imports/NT behavior and complete title entry remain blocked.')
if __name__=='__main__':main()
