#!/usr/bin/env python3
"""Maintainer CSD1 recursive critical-section ownership and audited fixtures; not title entry."""
import argparse,hashlib,importlib.util,json,os,subprocess,tarfile,tempfile
from pathlib import Path
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path,required=True);a=p.parse_args();app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 sdk=subprocess.check_output(['python3',str(app/'tools/check_native_sdk_identity.py'),'--app-lib',str(lib)],text=True).strip()
 pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert sha(kit)==pins['source_kit_sha256']
 with tarfile.open(kit)as archive:
  for item in pins['functions']:
   data=archive.extractfile(item['member']).read();assert hashlib.sha256(data).hexdigest()==item['member_sha256'];assert hashlib.sha256(data[item['start']:item['end']]).hexdigest()==item['sha256']
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('setup','../service-kernel-mixed-waits/setup.rs'),('probe','probe.rs'),('concurrency','concurrency.rs')]:modules.append((name,(here/relative).resolve()))
 sources={name:sha(path)for name,path in modules};tests={name:sha(here/name)for name in ['check.py','runtime.rs','admission.rs','reference_inputs.json']};out=a.evidence.resolve();out.mkdir(exist_ok=False);results=[]
 def declarations(paths):return '#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n'for name,path in paths)
 with tempfile.TemporaryDirectory(prefix='gamepkg-critical-sections-')as temp:
  root=Path(temp);source=root/'driver.rs';source.write_text(declarations(modules)+(here/'runtime.rs').read_text())
  for triple in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
   exe=root/(triple+('.exe'if'windows'in triple else''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(source),'-o',str(exe)];log=out/(triple+'.log')
   if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   execute='windows'not in triple or a.wine
   with log.open('w')as f:
    subprocess.run(cmd,check=True,stdout=f,stderr=subprocess.STDOUT)
    if execute:subprocess.run(['wine',str(exe)]if'windows'in triple else[str(exe)],check=True,stdout=f,stderr=subprocess.STDOUT,timeout=60,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   results.append({'target':triple,'execution':'Wine'if execute and'windows'in triple else'native Linux'if execute else'build-only','log_sha256':sha(log)});print('PASS critical sections '+triple,flush=True)
  original=(here/'../service-kernel-pool/kernel_pool.rs').read_text();variants=[]
  def change(name,begin,end,old,new):
   start=original.index(begin);stop=original.index(end,start)if end else len(original);part=original[start:stop];assert part.count(old)==1,(name,part.count(old));variants.append((name,original[:start]+part.replace(old,new,1)+original[stop:]))
  change('allow-foreign-unlock','    fn criticals_leave_locked(', '    pub fn criticals_close(', 'if !state.owned_by_caller || state.depth == 0 {','if false && (!state.owned_by_caller || state.depth == 0) {')
  change('forget-enter-count','    pub fn critical_wait_prepare(', '    pub fn critical_wait_slice(', '(pool_long(&bytes, 48)? as u32).wrapping_add(1)','pool_long(&bytes, 48)? as u32')
  change('forget-leave-count','    fn criticals_leave_locked(', '    pub fn criticals_close(', '(pool_long(&bytes, 56)? as u32).wrapping_add(1)','pool_long(&bytes, 56)? as u32')
  change('allow-prepare-replay','    pub fn critical_wait_prepare(', '    pub fn critical_wait_slice(', 'if self.closed || self.attempted {','if self.closed {')
  change('lose-busy-lease','    pub fn critical_wait_slice(', '    fn critical_wait_finish(', '            self.terminal = false;','            self.terminal = true;')
  change('skip-held-close-preflight','    fn criticals_close_locked(', 'impl CriticalCreation {', 'if state.locked {','if false && state.locked {')
  for name,text in variants:
   bad=out/(name+'.rs');bad.write_text(text);driver=root/(name+'.rs');driver.write_text(declarations([(n,bad if n=='kernel_pool'else path)for n,path in modules])+(here/'runtime.rs').read_text());exe=root/name;log=out/(name+'.log')
   with log.open('w')as f:
    subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(driver),'-o',str(exe)],check=True,stdout=f,stderr=subprocess.STDOUT);r=subprocess.run([str(exe)],stdout=f,stderr=subprocess.STDOUT,timeout=60)
   assert r.returncode!=0 and ('panicked at'in log.read_text()or'on an `Err` value:'in log.read_text()),name
   print('PASS required-failing '+name,flush=True)
  spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers);admit=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(admit)]
  for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True)
  with (out/'admission.log').open('w')as f:subprocess.run([str(admit),str(app),str(here),str(out)],check=True,stdout=f,stderr=subprocess.STDOUT)
  receipts={p.parent.name:sha(p)for p in out.glob('receipt-*/services.json')};assert len(receipts)==4
  assert sources=={n:sha(path)for n,path in modules}and tests=={n:sha(here/n)for n in tests},'sources changed during qualification'
  (out/'qualification.json').write_text(json.dumps({'schema':1,'reference_inputs':pins,'status':'passed-component-fixtures-not-title-entry','sdk':sdk,'sources':sources,'test_sources':tests,'results':results,'required_failing_native_copies':[name for name,_ in variants],'receipts':receipts,'linux':'actual native SDK workers and confined signed children','windows':'native SDK components under Wine if selected; signed artifacts build-only','profile_admission':'blocked','quotas':'unchanged'},indent=2)+'\n')
if __name__=='__main__':main()
