#!/usr/bin/env python3
"""Maintainer-only comparison against pinned original IRQ register save/restore."""
import argparse,importlib.util,hashlib,json,os,subprocess,tarfile,tempfile
from pathlib import Path
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--wine',action='store_true');p.add_argument('--app',type=Path);p.add_argument('--app-lib',type=Path);a=p.parse_args();
 if bool(a.app)!=bool(a.app_lib):p.error('--app and --app-lib are required together')
 here=Path(__file__).resolve().parent
 pin=json.loads((here/'irq_state_reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert hashlib.sha256(kit.read_bytes()).hexdigest()==pin['source_kit_sha256']
 with tarfile.open(kit) as t:data=t.extractfile(pin['member']).read()
 assert hashlib.sha256(data).hexdigest()==pin['source_sha256'];text=(here/'irq_state_reference.c.in').read_text()
 for name,part in pin['parts'].items():
  body=data[part['start']:part['end']];assert hashlib.sha256(body).hexdigest()==part['sha256'];text=text.replace('/* PINNED_'+name.upper()+' */',body.decode())
 assert '/* PINNED_' not in text
 with tempfile.TemporaryDirectory(prefix='gamepkg-irq-registers-') as temp:
  root=Path(temp);c=root/'reference.c';c.write_text(text)
  source=root/'tests.rs';driver=(here/'registers_tests.rs').read_text().replace('#[path = "registers.rs"]','#[path = '+json.dumps(str(here/'registers.rs'))+']')
  driver+='''\nunsafe extern "C"{fn irq_state_reference(before:*const u8,changed:*const u8,stack:u32,prepared:*mut u8,restored:*mut u8);}
#[test]fn pinned_original_irq_register_transitions(){for index in 0..4096u32{let before=sample(index);let changed=sample(index+0x10000);let mut prepared=[0u8;316];let mut restored=[0u8;316];unsafe{irq_state_reference(before.as_ptr(),changed.as_ptr(),0x12340000+index,prepared.as_mut_ptr(),restored.as_mut_ptr());}let mut state=registers::Registers::new();state.decode(&before).unwrap();state.irq_prepare(0x12340000+index);assert_eq!(state.encode(),prepared,"original prepare vector {index}");state.decode(&changed).unwrap();state.irq_restore(&before).unwrap();assert_eq!(state.encode(),restored,"original restore vector {index}");}}
'''
  source.write_text(driver)
  for target,cc,flags in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
   obj=root/(target+'.o');subprocess.run([cc,*flags,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(c),'-o',str(obj)],check=True)
   exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','--test','-O','--target',target,str(source),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   subprocess.run(cmd,check=True)
   if 'windows' not in target:subprocess.run([str(exe),'--test-threads=1'],check=True,timeout=60)
   elif a.wine:subprocess.run(['wine',str(exe),'--test-threads=1'],check=True,timeout=60,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
   else:print('PASS '+target+': compile only')
  bad=root/'regressed.rs';current=(here/'registers.rs').read_text();assert current.count('self.fs_base = holder_fs;')==1;bad.write_text(current.replace('self.fs_base = holder_fs;','self.fs_base = holder_fs.wrapping_add(1);'))
  negative=root/'negative.rs';negative.write_text(driver.replace(json.dumps(str(here/'registers.rs')),json.dumps(str(bad))))
  obj=root/'negative.o';subprocess.run(['gcc','-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(c),'-o',str(obj)],check=True);exe=root/'negative';subprocess.run(['rustc','--edition=2021','-D','warnings','--test','-O',str(negative),'-C','link-arg='+str(obj),'-o',str(exe)],check=True)
  result=subprocess.run([str(exe),'pinned_original_irq_register_transitions','--exact'],capture_output=True,text=True,timeout=60);assert result.returncode!=0 and 'original restore vector 0' in result.stdout+result.stderr,result.stdout+result.stderr
 if a.app_lib:
  lib=a.app_lib.resolve(strict=True);spec=importlib.util.spec_from_file_location('helpers',here.parent/'service-save/check.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
  with tempfile.TemporaryDirectory(prefix='gamepkg-irq-admission-') as temp:
   exe=Path(temp)/'admission';subprocess.run(['rustc','--edition=2021','-D','warnings',str(here/'irq_state_admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'--extern','tempfile='+str(h.dependency(lib,'tempfile')),'--extern','toml='+str(h.dependency(lib,'toml')),'-o',str(exe)],check=True);subprocess.run([str(exe),str(a.app.resolve(strict=True)),str(here)],check=True)
 print('PASS: pinned original IRQ register save/restore and prepare, 4096 vectors per target; corrupt-holder-FS control fails. Not IRQ delivery, gate/IRQL, callbacks or gameplay.')
if __name__=='__main__':main()
