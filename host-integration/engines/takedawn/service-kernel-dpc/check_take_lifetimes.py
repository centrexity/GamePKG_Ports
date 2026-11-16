#!/usr/bin/env python3
"""Maintainer-only actual SDK task lifetime and required-failing dequeue controls."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app',type=Path,required=True)
    parser.add_argument('--wine',action='store_true')
    parser.add_argument('--evidence',type=Path)
    args=parser.parse_args()
    app=args.app.resolve(strict=True);here=Path(__file__).resolve().parent
    modules=[('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs')]
    for name,relative in [('memory','../service-native/memory.rs'),('call_state','../service-native/call_state.rs'),('registers','../service-native/registers.rs'),('heap','../service-heap/heap.rs'),('contiguous','../service-contiguous/contiguous.rs'),('heap_service','../service-heap/service.rs'),('kernel_pool','../service-kernel-pool/kernel_pool.rs'),('wait_service','../service-kernel-wait/wait.rs'),('queue','queue.rs'),('cases','cases.rs'),('probe','probe.rs')]:
        modules.append((name,here/relative))
    with tempfile.TemporaryDirectory(prefix='gamepkg-locked-take-') as temp:
        root=Path(temp);prefix='#![allow(dead_code)]\n'+''.join('#[path='+json.dumps(str(path))+']mod '+name+';\n' for name,path in modules)
        source=root/'main.rs';source.write_text(prefix+(here/'take_lifecycle.rs').read_text())
        for target in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
            exe=root/(target+('.exe' if 'windows' in target else ''));cmd=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(source),'-o',str(exe)]
            if target=='i686-unknown-linux-gnu':cmd+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
            subprocess.run(cmd,check=True)
            if 'windows' not in target:subprocess.run([str(exe)],check=True,timeout=60)
            elif args.wine:subprocess.run(['wine',str(exe)],check=True,timeout=60,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
            else:print('PASS '+target+': compile only',flush=True)
        original=(here/'queue.rs').read_text();wrapper=source.read_text();controls=[]
        for name,old,new,expected in [
            ('foreign-reader','            || gamepkg::local_task_status()?.0 != self.worker\n','', 'wrong-task observation'),
            ('cancelled-owner-cleanup','if gamepkg::local_task_status()?.0 != self.worker {','if gamepkg::local_task_status()?.0 != self.worker || gamepkg::local_task_status()?.1 {','lifecycle join'),
        ]:
            assert original.count(old)==1
            bad=root/(name+'.rs');bad.write_text(original.replace(old,new));native=root/(name+'-main.rs');native.write_text(wrapper.replace(json.dumps(str(here/'queue.rs')),json.dumps(str(bad))))
            exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(native),'-o',str(exe)],check=True)
            result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=30)
            assert result.returncode!=0 and expected in result.stderr,result.stderr
            controls.append(name);print('PASS required failing '+name+' control; never admitted',flush=True)
        # Exercise publication order/cleanup with the checked fixture's error paths.
        fixture=root/'fixture.rs';fixture.write_text(prefix+'''mod gamepkg {pub use crate::client::*;}
fn worker(_:local::runtime::TaskContext,arg:u64)->u64{probe::dpc_worker(arg)}
fn main(){local::install(local::runtime::Runtime::new(&[local::runtime::Window{address:0x20000000,bytes:65536}],vec![worker],4*1024*1024,4,65536).unwrap()).unwrap();probe::dpc_controls().unwrap();probe::dpc_probe(false).unwrap();local::shutdown();}''')
        start=original.index('    fn take_commit(');stop=original.index('    // Attempts cleanup',start);part=original[start:stop]
        flag='        if value[0] != 0 {\n            ram.write_u8((value[0] as u32).wrapping_add(2), 0)?;\n        }\n'
        assert part.count(flag)==1
        moved=part.replace(flag,'').replace('        match gamepkg::local_record_compare_exchange',flag+'        match gamepkg::local_record_compare_exchange')
        for name,bad_source in [('flag-before-head',original[:start]+moved+original[stop:]),('missing-unlock',original.replace('let closed = self.take_abort();','let closed: Result<(),i32> = Ok(());'))]:
            bad=root/(name+'.rs');bad.write_text(bad_source);native=root/(name+'-main.rs');native.write_text(fixture.read_text().replace(json.dumps(str(here/'queue.rs')),json.dumps(str(bad))))
            exe=root/name;subprocess.run(['rustc','--edition=2021','-D','warnings','-O',str(native),'-o',str(exe)],check=True)
            result=subprocess.run([str(exe)],capture_output=True,text=True,timeout=30)
            assert result.returncode!=0 and 'on an `Err` value: -3' in result.stderr,result.stderr
            controls.append(name);print('PASS required failing '+name+' control; never admitted',flush=True)
        if args.evidence:
            args.evidence.mkdir(parents=True,exist_ok=False)
            report={'schema':1,'status':'actual-SDK-lifetimes-and-rejected-controls-not-gameplay','source_hashes':{name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in modules},'harness_sha256':hashlib.sha256((here/'take_lifecycle.rs').read_bytes()).hexdigest(),'actual_lifetimes_per_target':64,'required_failing_controls':controls,'windows':'Wine components' if args.wine else 'compile-only','scope':'maintainer shell; source/object admission and confined fixtures qualified separately'}
            (args.evidence/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')


if __name__=='__main__':
    main()
