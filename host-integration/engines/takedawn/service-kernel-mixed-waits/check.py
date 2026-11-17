#!/usr/bin/env python3
"""Maintainer mixed typed-owner qualification, not NT/title admission."""
import argparse,hashlib,importlib.util,json,os,shutil,subprocess,tarfile,tempfile
from pathlib import Path
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args();app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert sha(kit)==pins['source_kit_sha256']
 with tarfile.open(kit)as archive:original=archive.extractfile(pins['member']).read()
 assert hashlib.sha256(original).hexdigest()==pins['member_sha256'];body=original[pins['start']:pins['end']];assert hashlib.sha256(body).hexdigest()==pins['function_sha256']and(here/'reference.c').read_bytes().count(body)==1
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('mixed','mixed.rs'),('setup','setup.rs'),('probe','probe.rs'),('controls','controls.rs'),('concurrency','concurrency.rs')]:modules.append((name,here/relative))
 hashes={name:sha(path)for name,path in modules};tests={name:sha(here/name)for name in ['check.py','reference.c','reference_inputs.json','runtime.rs','fault_runtime.rs','admission.rs']}
 def declarations(paths):return '#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']'+('pub mod 'if name=='mixed'else'mod ')+name+';\n'for name,path in paths)
 with tempfile.TemporaryDirectory(prefix='gamepkg-mixed-owners-')as temp:
  root=Path(temp);expected=None
  for triple,compiler,flags in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(triple+'.o');subprocess.run([compiler,*flags,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(here/'reference.c'),'-o',str(obj)],check=True)
   for kind in ['runtime','fault_runtime']:
    source=root/(kind+'.rs');source.write_text(declarations(modules)+(here/(kind+'.rs')).read_text());exe=root/(kind+'-'+triple+('.exe'if'windows'in triple else''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(source),'-o',str(exe)]
    if kind=='runtime':cmd+=['-C','link-arg='+str(obj)]
    if kind=='runtime'and'windows'in triple:cmd+=['-C','link-arg=-lmingwex','-C','link-arg=-lmsvcrt','-C','link-arg=-lkernel32']
    if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
    subprocess.run(cmd,check=True)
    if'windows'in triple and not a.wine:continue
    prefix=['wine',str(exe)]if'windows'in triple else[str(exe)];env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine')
    if kind=='runtime':
     vector=root/(triple+'.vectors');arg='Z:'+str(vector).replace('/','\\')if'windows'in triple else str(vector);subprocess.run([*prefix,arg],check=True,env=env,timeout=120);data=vector.read_bytes();assert len(data)==2560
     if expected is None:expected=data
     else:assert data==expected,triple
    else:
     for mode in [*range(1,15),*range(20,25),*range(26,31),*range(40,55)]:subprocess.run([*prefix,str(mode)],check=True,env=env,timeout=30)
  (root/'vectors').write_bytes(expected)
  full=(here/'../service-kernel-pool/kernel_pool.rs').read_text();variants=[]
  def change(name,begin,end,old,new,kind='runtime',args=None):
   start=full.index(begin);stop=full.index(end,start)if end else len(full);part=full[start:stop];assert part.count(old)==1,(name,part.count(old));variants.append((name,full[:start]+part.replace(old,new,1)+full[stop:],kind,args or []))
  begin='// Port-owned mixed EVD2/SSD1 leases.'
  # Two loops intentionally exist: change only the validation loop.
  start=full.index(begin);part=full[start:];old='for (index, token) in tokens.iter().enumerate() {';assert part.count(old)==2
  variants.append(('skip-complete-validation',full[:start]+part.replace(old,'for (index, token) in tokens.iter().take(1).enumerate() {',1),'runtime',[]))
  change('hold-table-through-host-wait','    pub fn mixed_wait_prepare(', '    pub fn mixed_wait(', '        self.mixed_wait_unlock(ram)?;','        // Deliberately retain the table guard.')
  change('forget-accepted-event-lease','fn mixed_stage_event(', 'fn mixed_stage_semaphore(', '    lease.published = true;','    lease.published = false;', 'fault_runtime',['9'])
  change('forget-accepted-semaphore-lease','fn mixed_stage_semaphore(', 'impl MixedWait {','    lease.published = true;','    lease.published = false;', 'fault_runtime',['10'])
  change('forget-observed-native-result','    pub fn mixed_wait(', '    pub fn mixed_wait_native_result(', '        self.observed = true;','        self.observed = false;')
  change('allow-native-wait-replay','    pub fn mixed_wait(', '    pub fn mixed_wait_timeout(', '        self.waited = true;','        self.waited = false;')
  change('forget-event-completion-outcome','    fn mixed_load_event(', '    fn mixed_load_semaphore(', '            observed: bits & 8 != 0,','            observed: false,')
  change('forget-semaphore-completion-outcome','    fn mixed_load_semaphore(', '    fn mixed_save_event(', '            observed: bits & 8 != 0,','            observed: false,')
  change('hide-known-postwait-revocation','    pub fn mixed_wait(', '    pub fn mixed_wait_native_result(', '        let effective = match live {\n            Ok(_) => result,\n            Err(code) => Err(code),\n        };','        let effective = result;', 'fault_runtime',['5'])
  change('forget-postpublication-revocation','    pub fn mixed_wait_prepare(', '    pub fn mixed_wait(', '        self.mixed_wait_views(ram)?;\n        self.prepared = true;','        self.prepared = true;', 'fault_runtime',['11'])
  change('claim-close-with-owned-prefix','    pub fn mixed_wait_close(',None,'        if failure != 0 {\n            return Err(failure);\n        }','        let _ = failure;', 'fault_runtime',['7'])
  change('handles-forget-retirement','    pub fn handles_guard_closed(', '// Checked game adapter.','        self.left && !self.held','        !self.held')
  change('shadow-forget-retirement','    pub fn shadow_guard_closed(', '// Port-owned event/timer','        self.left && !self.held','        !self.held')
  change('handles-repeat-native-unlock','    pub fn handles_guard_close(', '    fn handles_leave(', '        if self.held {','        if self.held || !self.left {', 'fault_runtime',['8'])
  change('shadow-repeat-native-unlock','    pub fn shadow_guard_close(', '    fn shadow_leave(', '        if self.held {','        if self.held || !self.left {', 'fault_runtime',['12'])
  change('hide-nonstale-handle-count-error','    fn handles_leave(', '    pub fn handles_guard_closed(', '                Err(code) => return Err(code),','                Err(_) => continue,','fault_runtime',['26'])
  change('hide-nonstale-shadow-count-error','    fn shadow_leave(', '    pub fn shadow_guard_closed(', '                Err(code) => return Err(code),','                Err(_) => continue,','fault_runtime',['26'])
  old='        let result = gamepkg::local_wait_many(&objects[..self.count], all, milliseconds);'
  new='''        let result = if all {
            let mut sequential_error=0;
            for item in &objects[..self.count] {
                let step=if item.kind==1{gamepkg::local_event_wait(item.id,milliseconds)}else{gamepkg::local_semaphore_wait(item.id,milliseconds)};
                if let Err(code)=step{sequential_error=code;break;}
            }
            if sequential_error==0{Ok(gamepkg::LocalWaitResult{index:0,abandoned:false})}else{Err(sequential_error)}
        }else{gamepkg::local_wait_many(&objects[..self.count],false,milliseconds)};'''
  change('serial-waitall-consumes-prefix','    pub fn mixed_wait(', '    pub fn mixed_wait_native_result(',old,new)
  change('release-leases-on-busy-slice','    fn mixed_wait_step(', '    pub fn mixed_wait_timeout(', '            return Err(-2);','            return self.mixed_wait_complete(ram, Err(-2));')
  change('allow-logical-mode-change','    fn mixed_wait_step(', '    pub fn mixed_wait_timeout(', '|| (self.slicing && self.all != all)','|| false')
  change('replay-after-terminal-slice','    fn mixed_wait_step(', '    pub fn mixed_wait_timeout(', '        self.waited = true;','        self.waited = false;')
  change('hide-revocation-after-busy-slice','    fn mixed_wait_step(', '    pub fn mixed_wait_timeout(', 'result == Err(-2) && live.is_ok()','result == Err(-2)', 'fault_runtime',['49'])
  change('timeout-consumes-ready-signal','    pub fn mixed_wait_timeout(', '    fn mixed_wait_complete(', '        self.waited = true;','        let _ = self.mixed_wait_step(ram, self.all, 0, true);\n        self.waited = true;')
  change('timeout-hides-completion-error','    pub fn mixed_wait_timeout(', '    fn mixed_wait_complete(', '        self.mixed_wait_publish(ram)?;','        let _ = self.mixed_wait_publish(ram);','fault_runtime',['52'])
  change('timeout-hides-live-busy','    pub fn mixed_wait_timeout(', '    fn mixed_wait_complete(', '        match live {\n            Ok(_) => Ok(()),\n            Err(code) => Err(code),\n        }','        Ok(())','fault_runtime',['53'])
  broken={}
  for name,text,kind,args in variants:
   bad=root/(name+'.rs');bad.write_text(text);broken[name]=bad;source=root/(name+'-main.rs');source.write_text(declarations([(n,bad if n=='kernel_pool'else path)for n,path in modules])+(here/(kind+'.rs')).read_text());exe=root/name;cmd=['rustc','--edition=2021','-D','warnings','-O',str(source),'-o',str(exe)]
   if kind=='runtime':cmd+=['-C','link-arg='+str(root/'x86_64-unknown-linux-gnu.o')]
   subprocess.run(cmd,check=True);result=subprocess.run([str(exe),*args],capture_output=True,text=True,timeout=120);assert result.returncode!=0 and('assertion'in result.stderr or'on an `Err` value:'in result.stderr),(name,result.stderr)
   (root/(name+'.log')).write_text(result.stdout+result.stderr);print('PASS required-failing '+name+' native control',flush=True)
  spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers);admit=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(admit)]
  for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(admit),str(app),str(here),str(root)],check=True)
  admitted_bad=['serial-waitall-consumes-prefix','handles-forget-retirement','shadow-forget-retirement']
  for name in admitted_bad:
   out=root/('admitted-'+name);out.mkdir();shutil.copyfile(root/'vectors',out/'vectors');result=subprocess.run([str(admit),str(app),str(here),str(out),str(broken[name])],capture_output=True,text=True,timeout=180);assert result.returncode!=0 and'PASS Linux64: complete dependency-free native mixed-wait/helper source scanned before SDK injection'in result.stdout and'panicked at'in result.stderr and'BuildError'not in result.stderr,(name,result.stderr)
   (root/('admitted-'+name+'.log')).write_text(result.stdout+result.stderr);print('PASS audited admitted broken '+name+' Linux child',flush=True)
  assert hashes=={n:sha(path)for n,path in modules}and tests=={n:sha(here/n)for n in tests},'sources changed during qualification'
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',a.evidence/'vectors.bin');receipts={}
   for path in root.glob('receipt-*/services.json'):
    name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=sha(path)
   assert len(receipts)==4
   for name in broken:shutil.copyfile(broken[name],a.evidence/(name+'.rs'));shutil.copyfile(root/(name+'.log'),a.evidence/(name+'.log'))
   for path in root.glob('admitted-*.log'):shutil.copyfile(path,a.evidence/path.name)
   (a.evidence/'qualification.json').write_text(json.dumps({'schema':1,'status':'typed-mixed-EVD2-SSD1-ownership-not-NT-or-title-admission','original':pins,'sources':hashes,'test_sources':tests,'receipts':receipts,'vectors_sha256':sha(root/'vectors'),'vector_cases':128,'required_failing_native_copies':len(variants),'audited_confined_broken_copies':len(admitted_bad),'fault_cases_per_target':39,'linux':'native components and actual confined signed Linux32/64 children','windows':'native components Wine if requested; signed artifacts build-only','profile_admission':'blocked','quotas':'unchanged'},indent=2)+'\n')
 print('Mixed typed-owner component only; NT waits/close, mutex/file/thread owners and APC integration remain required; title blocked.')
if __name__=='__main__':main()
