#!/usr/bin/env python3
"""Maintainer typed semaphore owner/original-C qualification; not title admission."""
import argparse,hashlib,importlib.util,json,os,shutil,subprocess,tarfile,tempfile
from pathlib import Path

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app',type=Path,required=True)
    parser.add_argument('--app-lib',type=Path,required=True)
    parser.add_argument('--wine',action='store_true')
    parser.add_argument('--evidence',type=Path)
    args=parser.parse_args();app=args.app.resolve(strict=True);lib=args.app_lib.resolve(strict=True);here=Path(__file__).resolve().parent
    pins=json.loads((here/'reference_inputs.json').read_text());kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz';assert digest(kit)==pins['source_kit_sha256'];reference=(here/'reference.c').read_text()
    with tarfile.open(kit) as archive:
        scope=pins['comparison_scope'];assert hashlib.sha256(archive.extractfile(scope['posix_member']).read()).hexdigest()==scope['posix_sha256']
        for member,row in pins['members'].items():
            data=archive.extractfile(member).read();assert hashlib.sha256(data).hexdigest()==row['sha256']
            for name,pin in row['functions'].items():
                body=data[pin['start']:pin['end']];assert hashlib.sha256(body).hexdigest()==pin['sha256'] and reference.count(body.decode())==1,name
    modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
    for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('handles','../service-kernel-handles/handles.rs'),('semaphores','semaphores.rs'),('probe','probe.rs'),('wait_probe','wait_probe.rs')]:
        modules.append((name,here/relative))
    source_hashes={name:digest(path) for name,path in modules}
    tests=['check.py','runtime.rs','fault_runtime.rs','admission.rs','reference.c','reference_inputs.json'];test_hashes={name:digest(here/name)for name in tests}
    def declarations(paths):
        return '#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']'+('pub mod ' if name in ['handles','semaphores'] else'mod ')+name+';\n'for name,path in paths)
    with tempfile.TemporaryDirectory(prefix='gamepkg-native-semaphore-owner-')as temp:
        root=Path(temp);driver=root/'main.rs';driver.write_text(declarations(modules)+(here/'runtime.rs').read_text());fault=root/'fault.rs';fault.write_text(declarations(modules)+(here/'fault_runtime.rs').read_text());expected=None
        for target,compiler,extra in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
            obj=root/(target+'.o');subprocess.run([compiler,*extra,'-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(here/'reference.c'),'-o',str(obj)],check=True)
            for kind,source in [('oracle',driver),('fault',fault)]:
                exe=root/(kind+'-'+target+('.exe'if'windows'in target else''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-o',str(exe)]
                if kind=='oracle':cmd+=['-C','link-arg='+str(obj)]
                if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
                if kind=='oracle'and'windows'in target:cmd+=['-C','link-arg=-lmingwex','-C','link-arg=-lmsvcrt','-C','link-arg=-lkernel32']
                subprocess.run(cmd,check=True)
                if 'windows'in target and not args.wine:continue
                prefix=['wine',str(exe)]if'windows'in target else[str(exe)];env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine')
                if kind=='oracle':
                    vector=root/(target+'.vectors');argument='Z:'+str(vector).replace('/','\\')if'windows'in target else str(vector)
                    subprocess.run([*prefix,argument],env=env,check=True,timeout=120);data=vector.read_bytes();assert len(data)==1344
                    if expected is None:expected=data
                    else:assert expected==data,target
                else:subprocess.run(prefix,env=env,check=True,timeout=120)
        (root/'vectors').write_bytes(expected)
        implementation=here/'../service-kernel-pool/kernel_pool.rs';full=implementation.read_text();start=full.index('pub struct Semaphores {');end=full.index('// Checked original event bridges.',start);text=full[start:end]
        normal=[
            ('close-shared-alias','if references > 1 {','if references > 16383 {'),
            ('borrowed-owner-close','if !self.owning','if false'),
            ('discard-transferred-semaphore','if self.phase < 2 && !self.cleared {','if self.phase < 5 && !self.cleared {'),
            ('repeat-create','if self.attempted_create {','if self.attempted_create && false {'),
            ('allow-unowned-semaphore','        Err(-4)\n    }\n    pub fn semaphores_state(', '        Ok((0, value[0]))\n    }\n    pub fn semaphores_state('),
            ('shutdown-during-creation','if semaphores_word(&table, 32)? != 1 {','if semaphores_word(&table, 32)? == 0 {'),
            ('close-active-wait','        self.semaphores_wait_busy(id)?;',''),
            ('hold-table-through-wait','        self.semaphore_wait_unlock(ram)?;\n        // Never retain','        // Never retain'),
            ('ignore-actual-sdk-kind','                gamepkg::local_semaphore_state(value[0])?;',''),
            ('wrong-tag-kind','handle_guard_insert(ram, self.semaphore, 3, false)','handle_guard_insert(ram, self.semaphore, 1, false)'),
            ('query-cancelled-private-cleanup','                semaphores_release(self.semaphore)?;','                gamepkg::local_semaphore_state(self.semaphore)?;semaphores_release(self.semaphore)?;'),
        ]
        faults=[
            ('forget-transferred-prefix','self.phase = 2;','self.phase = 1;'),
            ('forget-released-prefix','self.released = true;','self.released = false;'),
            ('forget-published-wait','self.published = true;','self.published = false;'),
            ('forget-wait-outcome','self.observed = true;','self.observed = false;'),
            ('hide-accepted-creation-revocation','self.phase = 1;\n        semaphores_live(ram, self.root, self.services)?;','self.phase = 1;'),
            ('hide-accepted-release-revocation','closed?;\n        semaphores_live(ram, self.root, self.services)?;\n        Ok(previous)','closed?;\n        Ok(previous)'),
        ]
        for kind,changes in [('normal',normal),('fault',faults)]:
            for name,old,new in changes:
                assert text.count(old)==1,(name,text.count(old));bad=root/(name+'.rs');bad.write_text(full[:start]+text.replace(old,new,1)+full[end:]);source=root/(name+'-main.rs');source.write_text(declarations([(n,bad if n=='kernel_pool'else p)for n,p in modules])+(here/('runtime.rs'if kind=='normal'else'fault_runtime.rs')).read_text());exe=root/name
                cmd=['rustc','--edition=2021','-D','warnings','-O',str(source),'-o',str(exe)]
                if kind=='normal':cmd+=['-C','link-arg='+str(root/'x86_64-unknown-linux-gnu.o')]
                subprocess.run(cmd,check=True);result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=120)
                assert result.returncode!=0 and ('on an `Err` value:'in result.stderr or'assertion'in result.stderr),(name,result.stderr)
                print('PASS required-failing '+name+' native control; test shell never admitted',flush=True)
        spec=importlib.util.spec_from_file_location('helpers',here/'../service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers);exe=root/'admission';cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)]
        for name in ['tempfile','toml','serde_json']:cmd+=['--extern',name+'='+str(helpers.dependency(lib,name))]
        subprocess.run(cmd,check=True);subprocess.run([str(exe),str(app),str(here),str(root)],check=True)
        assert source_hashes=={name:digest(path)for name,path in modules}and test_hashes=={name:digest(here/name)for name in tests},'sources changed during qualification'
        if args.evidence:
            args.evidence.mkdir(parents=True,exist_ok=False);shutil.copyfile(root/'vectors',args.evidence/'vectors.bin');receipts={}
            for path in sorted(root.glob('receipt-*/services.json')):
                r=json.loads(path.read_text());assert len(r['game']['objects'])==len(r['raw_objects'])==8;name=path.parent.name+'.json';shutil.copyfile(path,args.evidence/name);receipts[name]=digest(path)
            assert len(receipts)==4
            report={'schema':1,'status':'checked-native-semaphore-owner-not-NT-import-or-title-admission','original':pins,'sources':source_hashes,'test_sources':test_hashes,'receipts':receipts,'vectors_sha256':digest(root/'vectors'),'required_failing_variants':len(normal)+len(faults),'fault_cases_per_target':13,'ledger_bytes':2624,'wait_slots':32,'windows':'Wine native components; signed artifact build-only'if args.wine else'build-only','linux':'standalone SDK and actual confined signed children','compatibility_scope':'original C success/count/previous-count witnesses; full NT status, guest ABI and actual import readiness remain unqualified','profile_admission':'blocked','remaining':'context/startup ownership and actual 193/222 imports; typed generic close/waits, full source/loop/renderer/media/build/launcher/gameplay'}
            (args.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Checked typed semaphores qualified; context/startup and actual NT/title entry still required.')

if __name__=='__main__':
    main()
