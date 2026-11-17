#!/usr/bin/env python3
"""Maintainer native event-owner qualification, not title/import admission."""
import argparse,hashlib,importlib.util,json,os,shutil,subprocess,tarfile,tempfile
from pathlib import Path
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args();app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert digest(kit)==pins['source_kit_sha256']
 with tarfile.open(kit) as archive:
  for member,pin in pins['members'].items():
   data=archive.extractfile(member).read();assert hashlib.sha256(data).hexdigest()==pin['sha256'];assert hashlib.sha256(data[pin['start']:pin['end']]).hexdigest()==pin['function_sha256']
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('handles','../service-kernel-handles/handles.rs'),('events','events.rs'),('probe','probe.rs'),('wait_probe','wait_probe.rs')]:modules.append((name,here/relative))
 with tempfile.TemporaryDirectory(prefix='gamepkg-native-event-owner-') as temp:
  root=Path(temp);driver=root/'main.rs';driver.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']'+('pub mod ' if name in ['handles','events'] else 'mod ')+name+';\n' for name,path in modules)+(here/'runtime.rs').read_text());expected=None
  for target in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(driver),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True);vector=root/(target+'.vectors')
   if 'windows' not in target:subprocess.run([str(exe),str(vector)],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe),'Z:'+str(vector).replace('/','\\')],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:continue
   data=vector.read_bytes();assert len(data)==48
   if expected is None:expected=data
   else:assert data==expected,target
  (root/'vectors').write_bytes(expected)

  for target in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   driver_fault=root/('fault-'+target+'.rs');driver_fault.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']'+('pub mod ' if name in ['handles','events'] else 'mod ')+name+';\n' for name,path in modules)+(here/'fault_runtime.rs').read_text())
   exe=root/('fault-'+target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(driver_fault),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe)],check=True,timeout=120)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=120,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
  implementation=here/'../service-kernel-pool/kernel_pool.rs';full=implementation.read_text();start=full.index('pub struct Events {');end=full.index('pub struct Semaphores {',start);prefix=full[:start];suffix=full[end:];text=full[start:end]
  mutations=[
   ('full-width-boolean','initial & 255 != 0','initial != 0'),
   ('wrong-manual-reset','event_type == 0','event_type != 0'),
   ('close-shared-alias','if references > 1 {','if references > 16383 {'),
   ('borrowed-owner-close','if !self.owning','if false'),
   ('discard-transferred-event','if self.phase < 2 && !self.cleared {','if self.phase < 5 && !self.cleared {'),
   ('repeat-create','if self.attempted_create {','if self.attempted_create && false {'),
   ('allow-unowned-event','        Err(-4)\n    }\n    pub fn events_state(', '        Ok((0, value[0]))\n    }\n    pub fn events_state('),
   ('shutdown-during-creation','if events_word(&table, 32)? != 1 {','if events_word(&table, 32)? == 0 {'),
   ('close-active-wait','        self.events_wait_busy(id)?;',''),
   ('block-signal-with-wait','        self.event_wait_unlock(ram)?;\n        // Never retain','        // Never retain'),
  ]
  for name,old,new in mutations:
   assert text.count(old)==1,(name,old);bad=root/(name+'.rs');bad.write_text(prefix+text.replace(old,new,1)+suffix);source=root/(name+'-main.rs');source.write_text(driver.read_text().replace(json.dumps(str(implementation)),json.dumps(str(bad))))
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(source),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=120);assert result.returncode!=0 and 'on an `Err` value:' in result.stderr,(name,result.stderr)
   print('PASS required-failing '+name+' native control; never admitted',flush=True)

  fault_mutations=[('forget-transferred-prefix','self.phase = 2;','self.phase = 1;'),('forget-released-prefix','self.released = true;','self.released = false;'),('forget-published-wait','self.published = true;','self.published = false;'),('forget-wait-outcome','self.observed = true;','self.observed = false;')]
  for name,old,new in fault_mutations:
   assert text.count(old)==1,(name,old);bad=root/(name+'.rs');bad.write_text(prefix+text.replace(old,new,1)+suffix);source=root/(name+'-fault.rs');source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(bad if mod=='kernel_pool' else path))+']'+('pub mod ' if mod in ['handles','events'] else 'mod ')+mod+';\n' for mod,path in modules)+(here/'fault_runtime.rs').read_text())
   exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(source),'-o',str(exe)],check=True)
   result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=120);assert result.returncode!=0 and ('assertion' in result.stderr or 'on an `Err` value:' in result.stderr),(name,result.stderr)
   print('PASS required-failing '+name+' native fault control; shim never admitted',flush=True)
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
   report={'schema':1,'status':'native-event-ownership-component-not-kernel-import-admission','original':pins,'sources':{name:digest(path) for name,path in modules},'test_sources':{name:digest(here/name) for name in ['check.py','runtime.rs','fault_runtime.rs','admission.rs','reference_inputs.json']},'receipts':receipts,'vectors_sha256':digest(root/'vectors'),'rustc':subprocess.check_output(['rustc','-vV'],text=True),'windows':'Wine components' if a.wine else 'compile-only','profile_admission':'blocked','required_failing_variants':len(mutations)+len(fault_mutations),'fault_cases_per_target':10,'ledger_bytes':2624,'quotas':'unchanged','remaining':'pulse, exact NT status/import mapping, new context/startup ownership, full native gameplay'}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('Actual SDK event component only; kernel imports/context/startup and actual title qualification remain required.')
if __name__=='__main__':main()
