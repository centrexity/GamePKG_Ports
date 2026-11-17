#!/usr/bin/env python3
"""Maintainer original-C/actual-SDK RTL import qualification; not title entry."""
import argparse,hashlib,importlib.util,json,os,re,shutil,subprocess,tarfile,tempfile
from pathlib import Path
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--wine',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args();app=a.app.resolve(strict=True);lib=a.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
 pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert sha(kit)==pins['source_kit_sha256'];reference=(here/'reference.c').read_text()
 with tarfile.open(kit)as archive:
  for member,pin in pins['members'].items():
   data=archive.extractfile(member).read();assert hashlib.sha256(data).hexdigest()==pin['sha256']
   for name,part in pin['functions'].items():
    body=data[part['start']:part['end']];assert hashlib.sha256(body).hexdigest()==part['sha256']and reference.count(body.decode())==1,name
  bridge=archive.extractfile('toolkit/src/kernel/kernel_bridge.c').read().decode()
  for ordinal,row in pins['ordinals'].items():
   assert len(re.findall(r'case\s+'+ordinal+r':\s*return\s+bridge_'+row['name']+r'\s*;',bridge))==1
   assert len(re.findall(r'case\s+'+ordinal+r':\s*return\s+'+str(row['argument_bytes'])+r'\s*;',bridge))==1
 modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
 for name,rel in [('memory','../service-native/memory.rs'),('registers','../service-native/registers.rs'),('call_state','../service-native/call_state.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('layout','../service-memory/layout.rs'),('rtl_probe','probe.rs'),('setup','setup.rs')]:modules.append((name,here/rel))
 hashes={name:sha(path)for name,path in modules};tests={n:sha(here/n)for n in ['check.py','reference.c','reference_inputs.json','runtime.rs','fault_runtime.rs','admission.rs']}
 def declarations(paths):return '#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n'for name,path in paths)
 with tempfile.TemporaryDirectory(prefix='gamepkg-native-rtl-')as temp:
  root=Path(temp);expected=None
  for triple,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(triple+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(here/'reference.c'),'-o',str(obj)],check=True)
   for kind in ['runtime','fault_runtime']:
    source=root/(kind+'.rs');source.write_text(declarations(modules)+(here/(kind+'.rs')).read_text());exe=root/(kind+'-'+triple+('.exe'if'windows'in triple else''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(source),'-o',str(exe)]
    if kind=='runtime':cmd+=['-C','link-arg='+str(obj)]
    if kind=='runtime'and'windows'in triple:cmd+=['-C','link-arg=-lmingwex','-C','link-arg=-lmsvcrt','-C','link-arg=-lkernel32']
    if triple=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
    subprocess.run(cmd,check=True)
    if'windows'in triple and not a.wine:continue
    prefix=['wine',str(exe)]if'windows'in triple else[str(exe)];env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine')
    if kind=='runtime':
     vector=root/(triple+'.vectors');arg='Z:'+str(vector).replace('/','\\')if'windows'in triple else str(vector);subprocess.run([*prefix,arg],check=True,env=env,timeout=120);data=vector.read_bytes();assert len(data)==2872
     if expected is None:expected=data
     else:assert data==expected
    else:
     for mode in range(1,6):subprocess.run([*prefix,str(mode)],check=True,env=env,timeout=120)
  (root/'vectors').write_bytes(expected)
  variants=[]
  for name,begin,end,old,new in [
   ('pending-as-unknown','fn rtl_status_error(','fn rtl_lower_byte(','0x103 => 997','0x103 => 317'),
   ('no-whole-word-floor','fn rtl_compare_ulong(','fn rtl_equal_string(','length & !3u32','length'),
   ('wrong-pattern-endianness','fn rtl_compare_ulong(','fn rtl_equal_string(','pool_word(&block, lane)? != pattern','pool_word(&block, lane)?.swap_bytes() != pattern'),
   ('word-count-instead-of-bytes','fn rtl_compare_ulong(','fn rtl_equal_string(','(done + lane as u64) as u32','((done + lane as u64) / 4) as u32'),
   ('full-u32-boolean','fn rtl_equal_string(','fn rtl_string_bytes(','insensitive as u8 != 0','insensitive != 0'),
   ('skip-null-buffer','fn rtl_equal_string(','fn rtl_string_bytes(','if left == 0 || right == 0 {','if false && (left == 0 || right == 0) {'),
   ('ignore-embedded-nul','fn rtl_equal_string(','fn rtl_string_bytes(','if av == 0 {','if av == 255 {'),
   ('wrong-string-clamp','fn rtl_init_string(',None,'.min(65534)','.min(65535)'),
   ('saturate-wide-maximum','fn rtl_init_string(',None,'bytes.wrapping_add(','bytes.saturating_add('),
  ]:variants.append((name,'kernel_pool',begin,end,old,new))
  for name,old,new in [
   ('cross-protected-page','(high - position).min(4096 - (offset & 4095))','{let _=offset; high - position}'),
   ('ignore-logical-span-end','(high - position).min(4096 - (offset & 4095))','4096 - (offset & 4095)'),
   ('partial-required-unit','& !(u64::from(unit) - 1)','& !0u64'),
   ('retry-known-read-error','self.read_bytes(address, &mut output[..count as usize])?;','if self.read_bytes(address,&mut output[..count as usize]).is_err(){self.read_bytes(address,&mut output[..count as usize])?;}'),
  ]:variants.append((name,'memory','    pub fn memory_read_chunk(','    pub fn read_bytes(',old,new))
  pool=(here/'../service-kernel-pool/kernel_pool.rs').read_text();start=pool.index('    ram.write_u16(destination, bytes)?;',pool.index('fn rtl_init_string('));stop=pool.index('    Ok(0)',start);old=pool[start:stop];variants.append(('rollback-accepted-header','kernel_pool','fn rtl_init_string(',None,old,'''    let mut header=std::vec::Vec::new();header.extend(u16::to_le_bytes(bytes));header.extend(u16::to_le_bytes(maximum));header.extend(u32::to_le_bytes(source));ram.write_bytes(destination,&header)?;
'''))
  bad_files={}
  for name,mod,begin,end,old,new in variants:
   path=next(path for n,path in modules if n==mod);text=path.read_text();start=text.index(begin);stop=text.index(end,start)if end else len(text);body=text[start:stop];assert body.count(old)==1,(name,body.count(old));bad=root/(name+'.rs');bad.write_text(text[:start]+body.replace(old,new,1)+text[stop:]);bad_files[name]=bad
   source=root/(name+'-main.rs');source.write_text(declarations([(n,bad if n==mod else path)for n,path in modules])+(here/('fault_runtime.rs'if name=='retry-known-read-error'else'runtime.rs')).read_text());exe=root/name;cmd=['rustc','--edition=2021','-D','warnings','-O',str(source),'-o',str(exe)]
   if name!='retry-known-read-error':cmd+=['-C','link-arg='+str(root/'x86_64-unknown-linux-gnu.o')]
   subprocess.run(cmd,check=True);result=subprocess.run([str(exe),*(['3']if name=='retry-known-read-error'else[])],capture_output=True,text=True,timeout=120);assert result.returncode!=0 and('assertion'in result.stderr or'on an `Err` value:'in result.stderr),(name,result.stderr)
   (root/(name+'.log')).write_text(result.stdout+result.stderr);print('PASS required-failing '+name+' actual-SDK control',flush=True)
  spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers);admit=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(admit)]
  for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
  subprocess.run(cmd,check=True);subprocess.run([str(admit),str(app),str(here),str(root)],check=True)
  for name in ['full-u32-boolean','rollback-accepted-header']:
   broken=root/('admitted-'+name);broken.mkdir();shutil.copyfile(root/'vectors',broken/'vectors')
   result=subprocess.run([str(admit),str(app),str(here),str(broken),str(bad_files[name])],capture_output=True,text=True,timeout=180);assert result.returncode!=0 and'PASS Linux64: complete dependency-free native RTL-import/helper source scanned before SDK injection'in result.stdout and'panicked at'in result.stderr and'BuildError'not in result.stderr,(name,result.stderr)
   (root/('admitted-'+name+'.log')).write_text(result.stdout+result.stderr);print('PASS audited admitted broken '+name+' Linux child',flush=True)
  assert hashes=={name:sha(path)for name,path in modules}and tests=={n:sha(here/n)for n in tests},'source changed during qualification'
  if a.evidence:
   a.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',a.evidence/'vectors.bin');receipts={}
   for path in root.glob('receipt-*/services.json'):
    name=path.parent.name+'.json';shutil.copyfile(path,a.evidence/name);receipts[name]=sha(path)
   assert len(receipts)==4
   for name in bad_files:
    shutil.copyfile(root/(name+'.log'),a.evidence/(name+'.log'));shutil.copyfile(bad_files[name],a.evidence/(name+'.rs'))
   for path in root.glob('admitted-*.log'):shutil.copyfile(path,a.evidence/path.name)
   (a.evidence/'qualification.json').write_text(json.dumps({'schema':1,'status':'actual-RTL-imports-not-title-admission','original':pins,'sources':hashes,'test_sources':tests,'receipts':receipts,'vectors_sha256':sha(root/'vectors'),'required_failing_native_copies':len(variants),'audited_confined_broken_copies':2,'fault_cases_per_target':5,'linux':'actual native components and confined signed children','windows':'native components Wine if requested; signed artifact build-only','profile_admission':'blocked'},indent=2)+'\n')
 print('Five actual RTL adapters only; complete native title entry remains blocked.')
if __name__=='__main__':main()
