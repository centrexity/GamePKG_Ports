#!/usr/bin/env python3
"""Maintainer original tagged-handle comparison; metadata IDs grant no native authority."""
import argparse,hashlib,importlib.util,json,os,shutil,subprocess,tarfile,tempfile
from pathlib import Path

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args();app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert digest(kit)==pins['source_kit_sha256']
 with tarfile.open(kit) as archive:original=archive.extractfile(pins['member']).read()
 assert hashlib.sha256(original).hexdigest()==pins['source_sha256']
 reference=(here/'reference.c').read_text()
 for name,pin in pins['functions'].items():
  body=original[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256'];assert reference.count(body.decode())==1,name
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('handles','handles.rs'),('cases','cases.rs'),('probe','probe.rs'),('controls','controls.rs')]:modules.append((name,here/relative))
 with tempfile.TemporaryDirectory(prefix='gamepkg-tagged-handles-') as temp:
  root=Path(temp);driver=root/'main.rs';driver.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+(here/'runtime.rs').read_text());expected=None
  for target,cc,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(target+'.o');subprocess.run([cc,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(here/'reference.c'),'-o',str(obj)],check=True)
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(driver),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   if 'windows' in target:cmd+=['-C','link-arg=-lmingwex','-C','link-arg=-lmsvcrt','-C','link-arg=-lkernel32']
   subprocess.run(cmd,check=True);vector=root/(target+'.vectors')
   if 'windows' not in target:subprocess.run([str(exe),str(vector)],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe),'Z:'+str(vector).replace('/','\\')],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:continue
   data=vector.read_bytes();assert len(data)==139256
   if expected is None:expected=data
   else:assert data==expected,target
  (root/'vectors').write_bytes(expected)
  for target in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   driver_fault=root/('fault-'+target+'.rs');driver_fault.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)+(here/'fault_runtime.rs').read_text())
   exe=root/('fault-'+target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(driver_fault),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe)],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
  text=(here/'handles.rs').read_text()
  for name,old,new in [
   ('truncate-object-id','if object == 0 || kind == 0 {\n            return Err(-1);\n        }\n        let mut free','let object=u64::from(object as u32); if object == 0 || kind == 0 {\n            return Err(-1);\n        }\n        let mut free'),
   ('reuse-fresh-duplicate','if !fresh && occupied == kind && value == object {','if (!fresh || fresh) && occupied == kind && value == object {'),
   ('allocate-before-dedup','if !fresh && occupied == kind && value == object {','if !fresh && occupied == kind && value == object && free[0] == 16 {'),
   ('ignore-object-kind','occupied == kind && value == object','occupied != 0 && value == object'),
   ('drop-page-owner','if handles_word(&b, 8)? != record {','if handles_word(&b, 8)? != record && false {'),
   ('close-active-guard','if handles_word(&b, 32)? != 0 {','if handles_word(&b, 32)? != handles_word(&b, 32)? {'),
   ('accept-raw-token','token & 0xff000000 != 0x48000000','token & 0xff000000 != token & 0xff000000'),
  ]:
   assert text.count(old)==1,(name,old);bad=root/(name+'.rs');bad.write_text(text.replace(old,new,1));source=root/(name+'-main.rs');source.write_text(driver.read_text().replace(json.dumps(str(here/'handles.rs')),json.dumps(str(bad))))
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(source),'-C','link-arg='+str(root/'x86_64-unknown-linux-gnu.o'),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=120);assert result.returncode!=0 and 'on an `Err` value:' in result.stderr,(name,result.stderr)
   print('PASS required failing '+name+' control; never admitted',flush=True)
  for name in ['forget-accepted-prefix','ignore-stale-memory','decrement-before-clear','publish-after-revoked-scan','bind-after-revocation']:
   mutated=text
   if name=='forget-accepted-prefix':
    start=text.index('    pub fn handles_bind(');end=text.index('    pub fn handles_borrow(',start);old=text[start:end]
    change=old.replace('self.initialized = index as u64 + 1;','let staged = index as u64 + 1;',1).replace(' + 1 != self.initialized',' + 1 != staged',1).replace('handles_put(&mut b, 64, self.initialized)?;','handles_put(&mut b, 64, staged)?;',1)
    marker='handles_put(&mut b, 64, staged)?;\n            handles_publish(self.record, version, &b)?;';assert change.count(marker)==1
    change=change.replace(marker,marker+'\n            self.initialized=staged;',1);mutated=text.replace(old,change,1)
   elif name=='ignore-stale-memory':
    start=text.index('fn handles_replace(');end=text.index('fn handles_decode(',start);old=text[start:end];assert old.count('Err(-3)')==1;mutated=text.replace(old,old.replace('Err(-3)','Ok(())'),1)
   elif name=='decrement-before-clear':
    old='handles_replace(self.window, self.offset, index, &page, &handles_zero)?;\n            self.initialized -= 1;';assert text.count(old)==1
    mutated=text.replace(old,'self.initialized -= 1;\n            handles_replace(self.window, self.offset, index, &page, &handles_zero)?;',1)
   elif name=='publish-after-revoked-scan':
    old='let slot = free[1];\n        self.handles_guard_snapshot(ram)?;';assert text.count(old)==1;mutated=text.replace(old,'let slot = free[1];',1)
   else:
    start=text.index('    pub fn handles_bind(');end=text.index('    pub fn handles_borrow(',start);old=text[start:end];assert old.count('handles_owner(ram, self.services)?;')==3
    change=old.replace('handles_owner(ram, self.services)?;','',3);mutated=text.replace(old,change,1)
   bad=root/(name+'.rs');bad.write_text(mutated);source=root/(name+'-fault.rs');source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(bad if mod=='handles' else path))+']mod '+mod+';\n' for mod,path in modules)+(here/'fault_runtime.rs').read_text())
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(source),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=120);assert result.returncode!=0 and 'assertion' in result.stderr,(name,result.stderr)
   print('PASS required failing '+name+' fault control; shim never admitted',flush=True)
  spec=importlib.util.spec_from_file_location('helpers',here/'../service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
  exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
  for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',a.evidence/'vectors.bin');receipts={}
   for path in sorted(root.glob('receipt-*/services.json')):
    record=json.loads(path.read_text());assert len(record['game']['objects'])==len(record['raw_objects'])==8
    name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=digest(path)
   assert len(receipts)==4
   report={'schema':1,'status':'tagged-handle-metadata-and-original-C-comparison-not-native-event-admission','original':pins,'sources':{name:digest(path) for name,path in modules},'receipts':receipts,'vectors_sha256':digest(root/'vectors'),'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows':'Wine components' if a.wine else 'compile-only','profile_admission':'blocked','required_failing_variants':12,'native_authority':'none; typed event/file/thread adapters still required','test_sources':{name:digest(here/name) for name in ['check.py','runtime.rs','fault_runtime.rs','reference.c','reference_inputs.json','admission.rs']},'metadata_root_bytes':80,'copied_records_quota':'unchanged:64 records/65536 bytes/4096 per record','scratch_bytes':262144,'fault_scope':'four-target native SDK components, Windows through Wine; shims rejected as game source'}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('Tagged handle metadata only; native resource ownership, kernel imports/context/startup and actual title qualification remain required.')
if __name__=='__main__':main()
