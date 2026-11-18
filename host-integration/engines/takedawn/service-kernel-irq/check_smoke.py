#!/usr/bin/env python3
"""Maintainer-only IRQ composition smoke checks, not IRQ/IRQL qualification."""
import argparse,importlib.util,json,os,subprocess,tempfile,hashlib,shutil
from pathlib import Path
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);p.add_argument('--fault-controls-only',action='store_true');a=p.parse_args();
 if a.fault_controls_only and (a.app_lib or a.evidence):p.error('fault-only checks do not build artifacts or issue qualification receipts')
 app=a.app.resolve(strict=True);here=Path(__file__).resolve().parent
 mods=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here.parent/'service-native/memory.rs'),('call_state',here.parent/'service-native/call_state.rs'),('registers',here.parent/'service-native/registers.rs'),('heap',here.parent/'service-heap/heap.rs'),('contiguous',here.parent/'service-contiguous/contiguous.rs'),('heap_service',here.parent/'service-heap/service.rs'),('kernel_pool',here.parent/'service-kernel-pool/kernel_pool.rs'),('gate',here.parent/'service-kernel-gate/gate.rs'),('irql',here.parent/'service-kernel-irql/state.rs'),('stacks',here.parent/'service-worker-stacks/stacks.rs'),('wait_service',here.parent/'service-kernel-wait/wait.rs'),('production_clock',here/'clock.rs'),('irq_delivery',here/'irq.rs'),('irq_hooks',here/'hooks.rs'),('irq_clock',here/'fixture_clock.rs'),('irq_routes',here/'fixture_routes.rs'),('irq_observer',here/'fixture_observer.rs'),('cases',here/'cases.rs'),('races',here/'races.rs'),('controls',here/'controls.rs'),('options',here/'options.rs'),('callbacks',here/'callbacks.rs'),('probe',here/'probe.rs')]
 with tempfile.TemporaryDirectory(prefix='gamepkg-irq-smoke-') as temp:
  root=Path(temp);source=root/'runtime.rs';
  # Checked test-copy pause only: exact production irq.rs has no after-mark hook.
  original=(here/'irq.rs').read_text();needle='let was = self.irq_mark_post(ram, line, now)?;';assert original.count(needle)==1
  instrumented=root/'irq_instrumented.rs';instrumented.write_text(original.replace(needle,needle+'\ncrate::irq_observer::after_mark_fixture(self,task,ram,cpu,line)?;'))
  irql_original=(here.parent/'service-kernel-irql/state.rs').read_text();start=irql_original.index('    pub fn irql_abort(');seam='            match self.task_commit(version, &b) {';at=irql_original.index(seam,start)
  irql_instrumented=root/'irql_instrumented.rs';irql_instrumented.write_text(irql_original[:at]+'            crate::irq_observer::abort_cas_fixture(self.controller,self.hook_context)?;\n'+irql_original[at:])
  mods=[(name,instrumented if name=='irq_delivery' else irql_instrumented if name=='irql' else path) for name,path in mods];source.write_text('#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n'for name,path in mods)+(here/'runtime.rs').read_text())
  for target in ([] if a.fault_controls_only else ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']):
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe)],check=True,timeout=60)
   elif a.wine:subprocess.run(['wine',str(exe)],check=True,timeout=60,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
  # Neither deliberately broken source is admitted as game code.
  wrapper=source.read_text()
  marker='if task.task_values(ram)?[3] == 0 {';assert original.count(marker)==2
  split=original.index(marker,original.index(marker)+len(marker))
  lost=original[:split]+original[split:].replace(marker,'if false && task.task_values(ram)?[3] == 0 {',1)
  lost=lost.replace(needle,needle+'\ncrate::irq_observer::after_mark_fixture(self,task,ram,cpu,line)?;')
  mutant=root/'lost_retry.rs';mutant.write_text(lost)
  native=root/'lost_retry_main.rs';native.write_text(wrapper.replace(json.dumps(str(instrumented)),json.dumps(str(mutant))))
  exe=root/'lost_retry';subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(native),'-o',str(exe)],check=True)
  result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=30)
  assert result.returncode!=0 and 'on an `Err` value: -3' in result.stderr,result.stderr
  print('PASS: required-failing omitted second-try control; mutant never admitted',flush=True)
  irql=irql_instrumented;current=irql.read_text()
  old='        if cpu.fs_base != 0 {\n            let address = cpu.fs_base.checked_add(0x24).ok_or(-1)?;\n            ram.write_u8(address, v[0] as u8)?;\n        }\n        if v[5] != 0 {\n            self.task_set(ram, 56, 0, false)?;\n            crate::irq_hooks::release_point(self, ram, cpu)?;\n        }'
  assert current.count(old)==1;split=old.index('        if v[5]')
  mutant=root/'early_release.rs';mutant.write_text(current.replace(old,old[split:]+'\n'+old[:split]))
  native=root/'early_release_main.rs';native.write_text(wrapper.replace(json.dumps(str(irql)),json.dumps(str(mutant))))
  exe=root/'early_release';subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(native),'-o',str(exe)],check=True)
  result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=30)
  assert result.returncode!=0 and 'on an `Err` value: -3' in result.stderr,result.stderr
  print('PASS: required-failing early PCR/release-point control; mutant never admitted',flush=True)
  current=instrumented.read_text()
  push='        ram.write_u32(cpu.esp, cpu.ebx)?;\n        task.irql_enable_after_shadow(ram, cpu)?;'
  assert current.count(push)==1
  mutant=root/'early_sti.rs';mutant.write_text(current.replace(push,'\n'.join(reversed(push.splitlines()))))
  native=root/'early_sti_main.rs';native.write_text(wrapper.replace(json.dumps(str(instrumented)),json.dumps(str(mutant))))
  exe=root/'early_sti';subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(native),'-o',str(exe)],check=True)
  result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=30)
  assert result.returncode!=0 and 'on an `Err` value: -3' in result.stderr,result.stderr
  print('PASS: required-failing STI release before the shadow stack store; mutant never admitted',flush=True)
  disable='        task.irql_disable(ram, cpu)?;'
  assert current.count(disable)==1
  mutant=root/'missing_cli.rs';mutant.write_text(current.replace(disable,''))
  native=root/'missing_cli_main.rs';native.write_text(wrapper.replace(json.dumps(str(instrumented)),json.dumps(str(mutant))))
  exe=root/'missing_cli';subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(native),'-o',str(exe)],check=True)
  result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=30)
  assert result.returncode!=0 and 'on an `Err` value: -3' in result.stderr,result.stderr
  print('PASS: required-failing omitted CLI before writes; mutant never admitted',flush=True)
  current=irql_instrumented.read_text();start=current.index('    pub fn irql_abort(')
  marker='Err(-2) => {} // Cleanup must not allocate a retry event under quota pressure.'
  at=current.index(marker,start);mutant=root/'quota_event_regression.rs';mutant.write_text(current[:at]+current[at:].replace(marker,'Err(-2) => memory::Memory::call_retry_pause()?,',1))
  native=root/'quota_event_main.rs';native.write_text(wrapper.replace(json.dumps(str(irql_instrumented)),json.dumps(str(mutant))))
  exe=root/'quota_event';subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(native),'-o',str(exe)],check=True)
  result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=30)
  assert result.returncode!=0 and 'on an `Err` value: -3131' in result.stderr,result.stderr
  print('PASS: required-failing event-dependent abort under full sync quota and forced stale CAS; mutant never admitted',flush=True)

  if a.app_lib:
   lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h);exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
   for name in ['tempfile','toml']:cmd+=['--extern',name+'='+str(h.dependency(lib,name))]
   subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);receipts={}
   for receipt in sorted(root.glob('receipt-*/services.json')):
    data=json.loads(receipt.read_text());assert len(data['game']['objects'])==8 and len(data['raw_objects'])==8
    name=receipt.parent.name+'.json';shutil.copyfile(receipt,a.evidence/name);receipts[name]=hashlib.sha256(receipt.read_bytes()).hexdigest()
   if a.app_lib:assert len(receipts)==4
   report={'schema':1,'status':'controlled-boundaries-qualified-not-production-IRQ','source_variant':'checked after-mark and once-stale-abort test copy','production_irq_sha256':hashlib.sha256(original.encode()).hexdigest(),'instrumented_irq_sha256':hashlib.sha256(instrumented.read_bytes()).hexdigest(),'production_irql_sha256':hashlib.sha256(irql_original.encode()).hexdigest(),'instrumented_irql_sha256':hashlib.sha256(irql_instrumented.read_bytes()).hexdigest(),'receipts':receipts,'scenarios':4,'repeats_each':32,'thread_patch_cases_per_runtime':128,'windows':'Wine' if a.wine else 'compile-only','required_failing_controls':['missing second gate attempt','release before PCR publication','STI release before the shadow stack store','CLI omitted before writes','event-dependent abort under sync quota and stale CAS'],'source_hashes':{name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in mods}}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('SMOKE ONLY: controlled native IRQ/IRQL/guest-stack composition and admission; four controlled worker interleavings; options/lifecycle/quota/callback boundaries included; production handlers/timer/DPC/gameplay remain unqualified.')
if __name__=='__main__':main()
