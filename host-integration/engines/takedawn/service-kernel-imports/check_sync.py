#!/usr/bin/env python3
"""Maintainer authenticated KIC4/KIC5 synchronization/import startup, not title entry."""
import argparse,hashlib,importlib.util,json,struct,subprocess,tempfile
from pathlib import Path

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--call-targets',type=Path,required=True);p.add_argument('--iso',type=Path);p.add_argument('--private-regular-only',action='store_true',help='With --iso, check the regular profile on the supplied image and keep all synthetic controls');p.add_argument('--evidence',type=Path);p.add_argument('--mutations',action='store_true');a=p.parse_args();app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 if a.private_regular_only and not a.iso:p.error("--private-regular-only requires --iso")
 private_modes=[96] if a.private_regular_only else [79,84,93,96] if a.iso else []
 def module(name,path):
  spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
 helpers=module('helpers',here.parent/'service-save/check.py');images=module('images',here.parent/'service-image/check.py');discs=module('discs',here.parent/'service-disc/check.py')
 with tempfile.TemporaryDirectory(prefix='gamepkg-sync-startup-')as temp:
  root=Path(temp);probe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'image_admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(probe)]
  for name in ['cap_std','tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);cases=[]
  for mode in [*range(69,79),80,81,82,83,85,86,87,88,89,90,91,92,94,95]:
   data=images.image();images.word(data,0x10c,0x770000);images.word(data,0x1b8+8,0x76e000);images.word(data,0x1b8+16,0x1000);images.word(data,0x158,0x12400^0x5b6d40b6);data[0xfc0:0x1000+379*4]=bytes(64+379*4)
   for index,ordinal in enumerate(([14,16,23,109,98,113,189,225,193,222]+([269,279,289,290,301] if mode==81 else [291,277,294] if mode==83 or mode==85 else [202,219,187]+([226] if mode==88 else [190,233,234] if mode==89 else [190,233,234,218]+([226] if mode==92 else [211,236,198,226,210,67,69,203,215,179,182,184,199,217] if mode>=94 else []) if mode>=90 else []) if mode>=86 else []))):images.word(data,0x1000+index*4,0x80000000|ordinal)
   images.word(data,0xffc,0x8000000e);image=root/(str(mode)+'.iso');discs.fixture(image)
   with image.open('r+b')as f:f.truncate(100*2048);f.seek(40*2048+4);f.write(struct.pack('<II',80,len(data)));f.seek(80*2048);f.write(data)
   cases.extend([str(mode),str(image)])
  if a.iso:
   iso_path=str(a.iso.resolve(strict=True))
   for mode in private_modes: cases.extend([str(mode),iso_path])
  prefix=[str(probe),str(app),str(here),str(a.call_targets.resolve(strict=True)),'--objects-layout'];evidence=[]
  if a.evidence:a.evidence.mkdir(parents=True,exist_ok=False);evidence=['--evidence',str(a.evidence.resolve())]
  subprocess.run([*prefix,*evidence,*cases],check=True)
  variants=[]
  if a.mutations:
   import re
   original=(here/'../service-kernel-context/startup.rs').read_text()
   old=re.search(r'kernel_pool::KernelContext::kernel_context_new_with_sync_objects\([\s\S]*?self\.semaphores\.as_ref\(\)\.ok_or\(-3\)\?\.semaphores_id\(\),\s*\)\?',original).group(0)
   variants.append(('omit-sync-context',old,'kernel_pool::KernelContext::kernel_context_new_with_objects(ram,self.registry.as_ref().ok_or(-3)?,self.shadow.as_ref().ok_or(-3)?,self.handles.as_ref().ok_or(-3)?.handles_id(),self.objects.as_ref().ok_or(-3)?.events_id())?',69))
   start=original.index('        if let Some(domain) = self.semaphores.as_mut() {');end=original.index('        if let Some(domain) = self.objects.as_mut()',start)
   drain=original[start:end]
   variants.append(('skip-semaphore-drain',drain,'        if self.semaphores.is_some(){self.semaphores=None;}\n',69))
   old='if ids\n                != [\n                    self.handles.as_ref().ok_or(-4)?.handles_id(),\n                    self.objects.as_ref().ok_or(-4)?.events_id(),\n                    owned.semaphores_id(),\n                ]'
   variants.append(('allow-substitute-semaphore-domain',old,old+' && false',71))
   variants.append(('forget-sync-job-join','if !self.jobs.is_empty() {','if false {',69))
   old='if !domain.semaphores_closed() {\n                return outcome;\n            }'
   variants.append(('forget-incomplete-semaphore-close',old,'if !domain.semaphores_closed(){self.close_error=0;}',74))
   old=re.search(r'self\.semaphores = Some\(kernel_pool::Semaphores::semaphores_new\([\s\S]*?\)\?\);',original).group(0)
   staged=old.replace('self.semaphores = Some(', 'let staged = ').replace(')?);', ')?;')
   # Retain only after context allocation: the failure loses the accepted SSD1.
   oldctx=re.search(r'Some\(\s*kernel_pool::KernelContext::kernel_context_new_with_sync_objects\([\s\S]*?\)\?,\s*\)',original).group(0)
   newctx=oldctx.replace('Some(', 'let staged_context = Some(',1).replace('self.semaphores.as_ref().ok_or(-3)?.semaphores_id()', 'staged.semaphores_id()')+'; self.semaphores=Some(staged); staged_context'
   combined=original.replace(old,staged,1).replace(oldctx,newctx,1)
   variants.append(('discard-failed-sync-context-prefix',original,combined,77))
   start=original.index('        if let Some(map) = self.handles.as_mut() {');end=original.index('        if let Some(map) = self.shadow.as_mut()',start)
   handles=original[start:end]
   reordered=original.replace(drain,'',1).replace(handles,handles+drain,1)
   variants.append(('destroy-table-before-semaphore-drain',original,reordered,69))
   start=original.index('    pub fn startup_prepare_with_criticals(');end=original.index('    fn startup_prepare_mode(',start)
   body=original[start:end]
   assert body.count('[scratch, offset, 2]')==1
   variants.append(('omit-critical-context',body,body.replace('[scratch, offset, 2]','[scratch, offset, 1]'),83))
   start=original.index('        if let Some(domain) = self.criticals.as_mut() {');end=original.index('        if let Some(map) = self.critical_shadow.as_mut()',start)
   body=original[start:end]
   variants.append(('discard-critical-native-obligations',body,'        self.criticals=None;\n',83))
   start=end;end=original.index('        if let Some(domain) = self.semaphores.as_mut()',start)
   body=original[start:end]
   variants.append(('discard-critical-map-obligations',body,'        self.critical_shadow=None;\n',83))
   for name,old,new,mode in variants:
    assert original.count(old)==1,(name,old);variant=root/(name+'.rs');variant.write_text(original.replace(old,new,1))
    result=subprocess.run([*prefix,'--linux64-only','--startup-copy',str(variant),str(mode),str(root/(str(mode)+'.iso'))],capture_output=True,text=True)
    log=result.stdout+result.stderr
    assert result.returncode!=0,(name,'broken startup accepted')
    assert 'PASS Linux64: complete image/import/helper source admission and all-object audit before trusted linking' in result.stdout,(name,result.stderr)
    assert 'error:' not in result.stderr and 'BuildError' not in result.stderr,(name,result.stderr)
    assert 'assertion' in result.stderr or 'panicked at' in result.stderr,(name,result.stderr)
    if a.evidence:(a.evidence/(name+'.log')).write_text(log);(a.evidence/(name+'.rs')).write_text(variant.read_text())
    print('PASS required failing '+name+' authenticated sync-startup control; no policy exemption',flush=True)
  if a.evidence:
   files={p.name:digest(p)for p in a.evidence.iterdir()if p.is_file()};sources={}
   for path in [here/'check_sync.py',here/'image_admission.rs',here/'image_probe.rs',here/'probe.rs',here/'../service-kernel-context/startup.rs',here/'../service-kernel-context/object_startup_probe.rs',here/'../service-kernel-context/sync_startup_probe.rs',here/'../service-kernel-pool/kernel_pool.rs',here/'../service-memory/layout.rs',here/'../service-native/memory.rs',here/'../service-kernel-handles/handles.rs',here/'../service-kernel-events/events.rs',here/'../service-kernel-event-imports/probe.rs',here/'../service-kernel-semaphore-imports/probe.rs',here/'../service-kernel-rtl/image_probe.rs',here/'../service-kernel-mixed-waits/image_probe.rs',here/'../service-critical/image_probe.rs',here/'../service-rom-handles/kernel_file.rs',here/'../service-rom-handles/kernel_drive.rs',here/'../service-rom-handles/rom_handles.rs',here/'../service-rom-handles/startup_file.rs',here/'../service-rom-handles/startup_probe.rs',here/'../service-save/kernel_directories.rs',*[here/'../service-save'/name for name in ['kernel_regular.rs','worker.rs','adapter.rs','files.rs','paths.rs','state.rs','history.rs','recovery.rs']],here/'../service-disc/disc.rs',here/'../service-image/image.rs',here/'imports.rs',here/'image_imports.rs']:sources[str(path.relative_to(here.parent))]=digest(path)
   receipts=[json.loads(p.read_text())for p in a.evidence.glob('receipt-*.json')];assert len(receipts)==4;sdk={r['game']['sdk']for r in receipts};assert len(sdk)==1
   (a.evidence/'qualification.json').write_text(json.dumps({'schema':1,'sdk':sdk.pop(),'status':'authenticated-KIC4-KIC5-startup-not-title-entry' if a.mutations else 'KIC4-KIC5-positive-startup-only-mutation-qualification-pending','sources':sources,'files':files,'synthetic_modes':[*range(69,79),80,81,82,83,85,86,87,88,89,90,91,92,94,95],'private_modes':private_modes,'required_failing_variants':len(variants),'profile_admission':'blocked','main_import_gap':'74 with regular save, drive links and a prepared virtual arena with queries; 86 with scoped optical and private directories; 93 with ordinary KIC5; 96 with KIC4; 98 with KIC3; original KIC2 checkpoint had 102','linux':'actual confined children','windows':'source/object/link only'},indent=2)+'\n')
 print('KIC4 startup/import checks passed; '+('ten required-failing startup controls passed; 'if a.mutations else'mutation qualification pending; ')+'full title remains blocked.')
if __name__=='__main__':main()
