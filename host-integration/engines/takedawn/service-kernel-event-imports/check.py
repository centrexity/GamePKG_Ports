#!/usr/bin/env python3
"""Maintainer original-C/native-SDK event import and signed-fixture qualification."""
import argparse,hashlib,importlib.util,json,os,shutil,subprocess,tarfile,tempfile
from pathlib import Path
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args();app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert digest(kit)==pins['source_kit_sha256'];reference=(here/'reference.c').read_text()
 with tarfile.open(kit) as archive:
  for member,pin in pins['members'].items():
   data=archive.extractfile(member).read();assert hashlib.sha256(data).hexdigest()==pin['sha256']
   for name,part in pin['functions'].items():
    body=data[part['start']:part['end']];assert hashlib.sha256(body).hexdigest()==part['sha256'];assert reference.count(body.decode())==1,name
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('layout','../service-memory/layout.rs'),('event_import_probe','probe.rs'),('setup','setup.rs')]:modules.append((name,here/relative))
 def declarations(paths):return '#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in paths)
 with tempfile.TemporaryDirectory(prefix='gamepkg-native-event-imports-') as temp:
  root=Path(temp);driver=root/'main.rs';driver.write_text(declarations(modules)+(here/'runtime.rs').read_text());fault=root/'fault.rs';fault.write_text(declarations(modules)+(here/'fault_runtime.rs').read_text());expected=None
  for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(here/'reference.c'),'-o',str(obj)],check=True)
   for kind,source in [('oracle',driver),('fault',fault)]:
    exe=root/(kind+'-'+target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-o',str(exe)]
    if kind=='oracle':cmd+=['-C','link-arg='+str(obj)]
    if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
    if kind=='oracle' and 'windows' in target:cmd+=['-C','link-arg=-lmingwex','-C','link-arg=-lmsvcrt','-C','link-arg=-lkernel32']
    subprocess.run(cmd,check=True)
    if 'windows' in target and not a.wine:continue
    env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine');prefix=['wine',str(exe)] if 'windows' in target else [str(exe)]
    if kind=='oracle':
     vector=root/(target+'.vectors');argument='Z:'+str(vector).replace('/','\\') if 'windows' in target else str(vector);subprocess.run([*prefix,argument],check=True,timeout=120,env=env);data=vector.read_bytes();assert len(data)==1764
     if expected is None:expected=data
     else:assert data==expected,target
    else:
     for mode in range(1,4):subprocess.run([*prefix,str(mode)],check=True,timeout=120,env=env)
  (root/'vectors').write_bytes(expected)
  implementation=here/'../service-kernel-pool/kernel_pool.rs';text=implementation.read_text()
  changes=[('full-width-boolean','initial & 255 != 0','initial != 0'),('wrong-resource-status','Ok(0xc000009a)','Ok(0)'),('forget-failed-output-zero','ram.write_u32(output, 0)?;','ram.write_u32(output, 0x33333333)?;'),('invalid-is-success','Ok(0xc0000008)','Ok(0)'),('forget-previous-zero','ram.write_u32(previous, 0)?;','ram.write_u32(previous, 0x55555555)?;'),('signal-before-previous-write','ram.write_u32(previous, 0)?;','let _=domain.events_set(ram,table,token,true);ram.write_u32(previous,0)?;'),('mask-metadata-failure','if domain.events_nt_signal(ram, table, token)? {','if domain.events_nt_signal(ram, table, token).unwrap_or(false) {'),('drop-context-classification',text[text.index('pub fn kernel_requires_context('):text.index('pub fn kernel_physical(')],'pub fn kernel_requires_context(ordinal:u32)->bool{ordinal==98||ordinal==113||ordinal==193||ordinal==222}\n')]
  # EventType is the third argument; ObjectAttributes must never supply its value.
  start=text.index('    if ordinal == 189 || ordinal == 225 {');end=text.index('    if ordinal == 113 {',start);block=text[start:end];assert block.count('pool_word(&args, 8)?')==1
  wrong_type=text[:start]+block.replace('pool_word(&args, 8)?','pool_word(&args,4)?',1)+text[end:]
  event_start=text.index('fn pool_nt_create_event(');event_end=text.index('fn pool_nt_create_semaphore(',event_start)
  def event_change(name,old,new):
   if name in ['wrong-resource-status','invalid-is-success','skip-failed-publication-close','hide-stale-as-resource-failure']:
    segment=text[event_start:event_end];assert segment.count(old)==1,(name,segment.count(old));return text[:event_start]+segment.replace(old,new,1)+text[event_end:]
   assert text.count(old)==1,(name,text.count(old));return text.replace(old,new,1)
  normal=[(name,event_change(name,old,new)) for name,old,new in changes];normal.append(('attributes-as-type',wrong_type))
  for name,mutated in normal:
   bad=root/(name+'.rs');bad.write_text(mutated);source=root/(name+'-main.rs');source.write_text(declarations([(mod,bad if mod=='kernel_pool' else path)for mod,path in modules])+(here/'runtime.rs').read_text());exe=root/name
   subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(source),'-C','link-arg='+str(root/'x86_64-unknown-linux-gnu.o'),'-o',str(exe)],check=True);result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=120);assert result.returncode!=0 and ('on an `Err` value:' in result.stderr or 'assertion' in result.stderr),(name,result.stderr)
   print('PASS required-failing '+name+' native original-C/import control; never admitted',flush=True)
  changes_fault=[('hide-revocation','if domain.events_nt_signal(ram, table, token)? {','if domain.events_nt_signal(ram, table, token).unwrap_or(false) {',1),('skip-failed-publication-close','    let published = incoming.event_publish(ram, output);\n    let closed = incoming.event_creation_close(ram);\n    published?;\n    closed?;','    incoming.event_publish(ram,output)?;incoming.event_creation_close(ram)?;',2),('hide-stale-as-resource-failure','    published?;\n    closed?;','    if published.is_err(){return Ok(0xc000009a);}closed?;',2)]
  for name,old,new,mode in changes_fault:
   bad=root/(name+'.rs');bad.write_text(event_change(name,old,new));source=root/(name+'-fault.rs');source.write_text(declarations([(mod,bad if mod=='kernel_pool' else path)for mod,path in modules])+(here/'fault_runtime.rs').read_text());exe=root/name
   subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(source),'-o',str(exe)],check=True);result=subprocess.run([str(exe),str(mode)],capture_output=True,text=True,timeout=120);assert result.returncode!=0 and ('on an `Err` value:' in result.stderr or 'assertion' in result.stderr),(name,result.stderr)
   print('PASS required-failing '+name+' actual-SDK fault control; shim never admitted',flush=True)
  spec=importlib.util.spec_from_file_location('helpers',here/'../service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers);exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
  for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',a.evidence/'vectors.bin');receipts={}
   for path in sorted(root.glob('receipt-*/services.json')):
    r=json.loads(path.read_text());assert len(r['game']['objects'])==len(r['raw_objects'])==8;name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=digest(path)
   assert len(receipts)==4
   report={'schema':1,'status':'actual-KIC3-NtCreateEvent-NtSetEvent-not-title-admission','imports':[189,225],'original':pins,'sources':{name:digest(path)for name,path in modules},'test_sources':{name:digest(here/name)for name in ['check.py','runtime.rs','fault_runtime.rs','reference.c','reference_inputs.json','admission.rs']},'receipts':receipts,'vectors_sha256':digest(root/'vectors'),'required_failing_variants':len(normal)+len(changes_fault),'fault_cases_per_target':3,'windows':'native SDK and original C through Wine; signed artifact build-only'if a.wine else'build-only','linux':'standalone native SDK and actual confined signed children; separate image qualification required','profile_admission':'blocked','remaining':'other kernel imports/NT waits/pulse and full source/loop/renderer/media/build/launcher/gameplay'}
   (a.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
 print('Two actual kernel imports only; complete native game qualification remains required.')
if __name__=='__main__':main()
