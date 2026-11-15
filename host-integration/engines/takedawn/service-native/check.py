#!/usr/bin/env python3
"""Maintainer-only conversion/differential/admission checks. Never required by players."""
import argparse, hashlib, importlib.util, json, os
from pathlib import Path
import subprocess, tarfile, tempfile, re

KIT = "6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4"
UNIT = "prepared/src/recomp_0000_001.rs"
UNIT_HASH = "442f7812516f3c5157ee74e15aede23d0ac423105acb7d973733bbe9f01ad94d"

def pinned_function(text, prefix):
    # Exact hash-pinned generated units only. Reject brace-bearing strings or
    # comments rather than treating their contents as structural delimiters.
    literals=r'"(?:\\.|[^"\\])*"'
    assert not any('{' in token or '}' in token for token in re.findall(literals,text))
    assert text.count(prefix)==1
    start=text.index(prefix);opening=text.index('{',start);end=opening+1;depth=1
    while depth:
        depth+=(text[end]=='{')-(text[end]=='}');end+=1
    body=text[start:end];outside=re.sub(literals,'',text[opening+1:end])
    assert not any(marker in outside for marker in ['//','/*',"'{'","'}'"])
    return body

def prepare_debug_reference(here,kit,scratch):
    pin=json.loads((here/'maintainer/reviewed_debug_service.json').read_text())
    assert pin['source_kit_sha256']==KIT
    with tarfile.open(kit) as archive:
        data=archive.extractfile(pin['member']).read()
    assert hashlib.sha256(data).hexdigest()==pin['source_sha256']
    body=data[pin['function_start']:pin['function_end']]
    assert hashlib.sha256(body).hexdigest()==pin['function_sha256']
    assert body.startswith(b'void recomp_debug_service(uint32_t service, uint32_t arg_va)')
    template=(here/'debug_reference.c.in').read_text()
    assert template.count('/* GAMEPKG_PINNED_ORIGINAL_BODY */')==1
    output=scratch/'debug_reference.c'
    output.write_text(template.replace('/* GAMEPKG_PINNED_ORIGINAL_BODY */',body.decode()))
    return output

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app',type=Path,required=True)
    parser.add_argument('--app-lib',type=Path,required=True)
    parser.add_argument('--prepared',type=Path,help='whole-kit candidate directory to source-check and compile in place of the single unit')
    parser.add_argument('--call-targets',type=Path,required=True,help='verified original-target classifier and lookup vectors to admit alongside game source')
    parser.add_argument('--reference-only',action='store_true',help='maintainer diagnostics only: skip source/object admission, linking and confined execution; never production qualification')
    parser.add_argument('--wine',action='store_true',help='also execute Windows comparison binaries under Wine')
    args=parser.parse_args()
    here=Path(__file__).resolve().parent;app=args.app.resolve(strict=True);library=args.app_lib.resolve(strict=True)
    # Reuse exact app fingerprint resolution, not the newest potentially different feature build.
    spec=importlib.util.spec_from_file_location('save_checks',here.parent/'service-save/check.py')
    helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
    target=library.parent.parent.parent
    env=dict(os.environ,CARGO_TARGET_DIR=str(target))
    manifest=here/'maintainer/Cargo.toml'
    subprocess.run(['cargo','test','--locked','--offline','--manifest-path',str(manifest)],env=env,check=True)
    if args.wine:
        for test in ['native_control_flow_matches','compacted_native_reference','guarded_native_reference']:
            subprocess.run(['cargo','test','--locked','--offline','--manifest-path',str(manifest),test,'--','--ignored','--test-threads=1'],env=env,check=True)
    subprocess.run(['cargo','build','--locked','--offline','--manifest-path',str(manifest)],env=env,check=True)
    tool=target/'debug/takedawn-safe-source-preparation'
    kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
    if hashlib.sha256(kit.read_bytes()).hexdigest()!=KIT:raise RuntimeError('Changed source kit requires review')
    with tempfile.TemporaryDirectory(prefix='gamepkg-native-source-') as name:
        scratch=Path(name)
        with tarfile.open(kit) as archive:
            original=archive.extractfile(UNIT).read()
            extra_helpers=[]
            extra_original=""
            icall_original=""
            split_original=""
            fs_original=""
            loop_original=""
            loop_support=""
            debug_original=""
            helper_units=[('prepared/src/recomp_0000_000.rs',['XMM_SUB','XMM_XOR']),
                          ('prepared/src/recomp_0002_018.rs',['recomp_fp_round24']),
                          ('prepared/src/recomp_0004_010.rs',['recomp_parity8']),
                          ('prepared/src/recomp_0007_000.rs',['RC_ROT']),
                          ('prepared/src/recomp_0007_001.rs',['recomp_frndint','recomp_fist'])]
            pins=dict(line.split('\t') for line in (here/'maintainer/reviewed_units.tsv').read_text().splitlines())
            manual_data=archive.extractfile('prepared/src/recomp_manual.rs').read()
            assert hashlib.sha256(manual_data).hexdigest()==pins['recomp_manual.rs']
            for helper in ['recomp_icall_not_code_log','recomp_icall_fail_log']:
                extra_helpers.append(pinned_function(manual_data.decode(),'pub unsafe extern "C" fn '+helper+'('))
            if args.prepared:
                debug_data=archive.extractfile('prepared/src/recomp_0004_014.rs').read()
                assert hashlib.sha256(debug_data).hexdigest()==pins['recomp_0004_014.rs']
                debug_original=pinned_function(debug_data.decode(),'pub unsafe extern "C" fn sub_001D2955(')
                icall_data=archive.extractfile('prepared/src/recomp_0005_013.rs').read()
                assert hashlib.sha256(icall_data).hexdigest()==pins['recomp_0005_013.rs']
                icall_original=pinned_function(icall_data.decode(),'pub unsafe extern "C" fn sub_001FBAE0(')
                split_data=archive.extractfile('prepared/src/recomp_0006_019.rs').read()
                assert hashlib.sha256(split_data).hexdigest()==pins['recomp_0006_019.rs']
                split_original=pinned_function(split_data.decode(),'pub unsafe extern "C" fn sub_00243450(')

            for unit_name,helpers_in_unit in helper_units:
                data=archive.extractfile(unit_name).read()
                assert hashlib.sha256(data).hexdigest()==pins[Path(unit_name).name]
                extra_unit=data.decode()
                for helper in helpers_in_unit:
                    found=re.search(r'(?m)^#\[inline\]\nunsafe extern "C" fn '+helper+r'\(',extra_unit)
                    start=found.start();opening=extra_unit.index('{',start);end=opening+1;depth=1
                    # Exact kit-pinned bodies: no braces in literals/comments.
                    while depth:
                        depth+=(extra_unit[end]=='{')-(extra_unit[end]=='}');end+=1
                    extra_helpers.append(extra_unit[start:end])
                if args.prepared and unit_name=="prepared/src/recomp_0007_000.rs":
                    fs_original=pinned_function(extra_unit,'pub unsafe extern "C" fn sub_002447BF(')
                    loop_original=pinned_function(extra_unit,'pub unsafe extern "C" fn sub_0024490A(')
                    loop_support=pinned_function(extra_unit,'pub unsafe extern "C" fn sub_0024499E(')
                if args.prepared and unit_name=="prepared/src/recomp_0007_001.rs":
                    extra_original=pinned_function(extra_unit,'pub unsafe extern "C" fn sub_00244DF0(')
        debug_source=prepare_debug_reference(here,kit,scratch)
        if hashlib.sha256(original).hexdigest()!=UNIT_HASH:raise RuntimeError('Changed original unit')
        unit=scratch/'original.rs';unit.write_bytes(original)
        generated=scratch/'generated'
        subprocess.run([str(tool),str(unit),str(generated)],check=True)
        for filename in ['game.rs','dispatch.rs','provenance.json']:
            if (generated/filename).read_bytes()!=(here/'generated'/filename).read_bytes():
                raise RuntimeError(f'Checked-in generated {filename} is stale')
        record=json.loads((generated/'provenance.json').read_text())
        if len(record['converted'])!=32 or len(record['rejected'])!=18:raise RuntimeError('Unexpected conversion coverage')
        changed=scratch/'changed.rs';changed.write_bytes(original+b'\n')
        denied=subprocess.run([str(tool),str(changed),str(scratch/'must-not-exist')],capture_output=True)
        assert denied.returncode!=0 and not (scratch/'must-not-exist').exists()
        probe=scratch/'admission'
        command=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern',f'gamepkg={library}','-L',f'dependency={library.parent}','-o',str(probe)]
        for name in ['cap_std','tempfile','toml','serde_json']:
            command+=['--extern',f'{name}={helpers.dependency(library,name)}']
        subprocess.run(command,check=True)
        prepared=[str(args.prepared.resolve(strict=True))] if args.prepared else []
        if args.call_targets:
            prepared=prepared or ['-']
            prepared.append(str(args.call_targets.resolve(strict=True)))
        # The complete harness below repeats source admission and unused-reference
        # rejection before binding injection. Avoid a duplicate whole-kit walk;
        # keep the small early diagnostic for the default single-unit fixture.
        if not args.prepared:
            subprocess.run([str(probe),str(app),str(here),"--source-only",*prepared],check=True)
        names=list(record['converted'])
        game_file=generated/'game.rs';dispatch_file=generated/'dispatch.rs'
        if args.prepared:
            report=json.loads((args.prepared/'inventory.json').read_text());selected={};locations={}
            wanted={'sub_00244DF0':'recomp_0007_001.rs','sub_001FBAE0':'recomp_0005_013.rs','sub_00243450':'recomp_0006_019.rs','sub_002447BF':'recomp_0007_000.rs','sub_0024490A':'recomp_0007_000.rs','sub_0024499E':'recomp_0007_000.rs','sub_001D2955':'recomp_0004_014.rs'}
            for name,unit_name in wanted.items():
                entry=report['closed_graph'][name];assert entry['unit']==unit_name and entry['dependencies']==(['sub_0024499E'] if name=='sub_0024490A' else [])
            for filename,expected in report['emitted_files'].items():
                if not re.fullmatch(r'game/unit_[0-9]+\.rs',filename):continue
                data=(args.prepared/filename).read_bytes();assert hashlib.sha256(data).hexdigest()==expected
                text=data.decode()
                for name in wanted:
                    if 'pub fn '+name+'(' in text:
                        assert name not in selected;selected[name]=pinned_function(text,'pub fn '+name+'(')
                        locations[name]=Path(filename).stem
            assert set(selected)==set(wanted) and extra_original and icall_original and split_original and fs_original and loop_original and loop_support
            # This small reference crate flattens only the selected, hash-checked
            # literal dependency. The admitted full tree retains its original
            # partition-qualified calls and every source/object check.
            for name in selected:
                for dependency in report['closed_graph'][name]['dependencies']:
                    assert dependency in selected
                    call=f'crate::game::{locations[dependency]}::{dependency}('
                    assert selected[name].count(call)==1
                    selected[name]=selected[name].replace(call,f'crate::game::{dependency}(')
            game_file=scratch/'combined_game.rs';game_file.write_text((generated/'game.rs').read_text()+'\n'+'\n'.join(selected.values()))
            dispatch_file=scratch/'combined_dispatch.rs'
            branches=''.join('0x'+name[4:]+'=>crate::game::'+name+'(native_state,guest_ram),' for name in selected)
            dispatch_file.write_text('pub fn invoke(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory,address:u32)->Result<(),i32>{if !native_state.valid(){return Err(-1);}match address{'+branches+'_=>crate::base_dispatch::invoke(native_state,guest_ram,address)}}')
            names+=[n for n in selected if n not in ['sub_0024490A','sub_0024499E','sub_001D2955']]
            names+=['sub_0024490A','sub_001D2955']
        oracle=here.joinpath('oracle_prefix.inc.rs').read_text()+'\n'+generated.joinpath('oracle_helpers.rs').read_text()+'\n'+'\n'.join(extra_helpers)+'\n'+generated.joinpath('oracle.rs').read_text()+'\n'+extra_original+'\n'+icall_original+'\n'+split_original+'\n'+fs_original+'\n'+loop_original+'\n'+loop_support+'\n'+debug_original
        oracle+='\npub unsafe fn invoke(address:u32){match address{'+''.join(f'0x{n[4:]}=>{n}(),' for n in names)+'_=>panic!("unknown reference function")}}\n'
        oracle+='\npub unsafe fn helper(operation:u32,first:[u32;4],second:[u32;4],immediate:u32)->[u32;4]{let a=RecompXmm{u:first};let b=RecompXmm{u:second};let result=match operation{0=>XMM_ADD(a,b),1=>XMM_SUB(a,b),2=>XMM_MUL(a,b),3=>XMM_XOR(a,b),4=>XMM_SHUFFLE(a,b,immediate),5=>XMM_SCALAR(f32::from_bits(first[0])),6=>XMM_ZERO(),_=>panic!("unknown helper")};result.u}\n'
        oracle+='\npub unsafe fn scalar_round24(value:f64)->f64{recomp_fp_round24(value)}pub unsafe fn scalar_parity8(value:u32)->i32{recomp_parity8(value)}pub unsafe fn scalar_rotate(value:u32,count:u32,carry:&mut i32,width:u32,left:bool)->u32{RC_ROT(value,count,carry,width,i32::from(left))}\n'
        oracle+='\npub unsafe fn scalar_round_integer(value:f64,control:u16)->f64{recomp_frndint(value,control)}pub unsafe fn scalar_integer_store(value:f64,control:u16,width:u32)->i64{recomp_fist(value,control,width)}\n'
        oracle_file=scratch/'reference.rs';oracle_file.write_text(oracle)
        wrapper='#![allow(non_camel_case_types,non_snake_case,non_upper_case_globals,unused_assignments,unused_variables,unused_mut,unused_parens,dead_code,unreachable_code)]\n'
        for module,path in [('client',app/'engines/native-services/client.rs'),('local',app/'engines/native-services/local_bindings.rs'),('memory',here/'memory.rs'),('call_state',here/'call_state.rs'),('copy_checks',here/'copy_checks.rs'),('fill_checks',here/'fill_checks.rs'),('debug',here/'debug.rs'),('debug_checks',here/'debug_checks.rs'),('fs_checks',here/'fs_checks.rs'),('jump_checks',here/'jump_checks.rs'),('vector',here/'vector.rs'),('scalar',here/'scalar.rs'),('registers',here/'registers.rs'),('game',game_file),('base_dispatch',generated/'dispatch.rs'),('dispatch',dispatch_file),('oracle',oracle_file)]:
            wrapper+=f'#[path={json.dumps(str(path))}]mod {module};\n'
        wrapper+=here.joinpath('reference_call_events.rs').read_text()+'\n'
        if args.call_targets:
            target_report=json.loads((args.call_targets/'call_targets.json').read_text())
            for filename,expected in target_report['emitted_files'].items():
                data=(args.call_targets/filename).read_bytes();assert hashlib.sha256(data).hexdigest()==expected
                dest=scratch/filename;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
            wrapper+='mod targets;\n#[path='+json.dumps(str(here/'calls_runtime.rs'))+']mod calls_runtime;\n'
        elif args.prepared:
            raise RuntimeError('Runtime call qualification requires --call-targets')
        wrapper+=(here/'runtime_guard_checks.rs').read_text() if args.call_targets else 'fn compare_runtime_guards() {}\n'
        wrapper+=f'const ADDRESSES:[u32;{len(names)}]=['+','.join('0x'+n[4:] for n in names)+'];\n'
        wrapper+=here.joinpath('debug_reference.inc.rs').read_text()+'\n'
        wrapper+=here.joinpath('comparison.rs').read_text()
        main_rs=scratch/'main.rs';main_rs.write_text(wrapper)
        vectors=scratch/'vectors'
        for triple in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
            exe=scratch/('compare-'+triple+('.exe' if 'windows' in triple else ''))
            command=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(main_rs),'-o',str(exe)]
            if triple=='i686-unknown-linux-gnu':command+=['-C','linker=/usr/bin/gcc-11']
            state_exe=scratch/('call-state-'+triple+('.exe' if 'windows' in triple else ''))
            state_command=['rustc','--edition=2021','-D','warnings','--test','-O','--target',triple,str(here/'call_state_tests.rs'),'-o',str(state_exe)]
            if triple=='i686-unknown-linux-gnu':state_command+=['-C','linker=/usr/bin/gcc-11']
            subprocess.run(state_command,check=True)
            if 'windows' not in triple:
                subprocess.run([str(state_exe),'--test-threads=1'],check=True)
            elif args.wine:
                subprocess.run(['wine',str(state_exe),'--test-threads=1'],check=True,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'),timeout=60)
            registers_exe=scratch/('registers-'+triple+('.exe' if 'windows' in triple else ''))
            registers_command=['rustc','--edition=2021','-D','warnings','--test','-O','--target',triple,str(here/'registers_tests.rs'),'-o',str(registers_exe)]
            if triple=='i686-unknown-linux-gnu':registers_command+=['-C','linker=/usr/bin/gcc-11']
            subprocess.run(registers_command,check=True)
            if 'windows' not in triple:subprocess.run([str(registers_exe),'--test-threads=1'],check=True)
            elif args.wine:subprocess.run(['wine',str(registers_exe),'--test-threads=1'],check=True,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'),timeout=60)
            reference_object=scratch/('icall-reference-'+triple+'.o')
            compiler={'x86_64-unknown-linux-gnu':['gcc'],'i686-unknown-linux-gnu':['/usr/bin/gcc-11','-m32'],'x86_64-pc-windows-gnu':['x86_64-w64-mingw32-gcc'],'i686-pc-windows-gnu':['i686-w64-mingw32-gcc']}[triple]
            subprocess.run(compiler+['-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(here/'icall_reference.c'),'-o',str(reference_object)],check=True)
            command+=['-C','link-arg='+str(reference_object)]
            debug_object=scratch/('debug-reference-'+triple+'.o')
            subprocess.run(compiler+['-std=c11','-Wall','-Wextra','-Werror','-O2','-c',str(debug_source),'-o',str(debug_object)],check=True)
            command+=['-C','link-arg='+str(debug_object)]
            if 'windows' in triple:command+=['-C','link-arg=-Wl,--start-group','-C','link-arg=-lmingwex','-C','link-arg=-lmsvcrt','-C','link-arg=-lkernel32','-C','link-arg=-Wl,--end-group'] # reference-only C stdio implementation, after its object
            subprocess.run(command,check=True)
            if 'windows' not in triple:
                subprocess.run([str(exe)]+([str(vectors)] if triple.startswith('x86_64') else []),check=True,timeout=180)
            elif args.wine:
                subprocess.run(['wine',str(exe)],check=True,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'),timeout=180)
            else:print(f'PASS {triple}: compilation only; runtime not exercised',flush=True)
        if args.reference_only:
            print('Reference execution only: source/object admission, trusted linking and confined game execution NOT performed. Not production qualification.')
            return
        subprocess.run([str(probe),str(app),str(here),str(vectors),*prepared],check=True)
    print('Component evidence only: no complete generated game, ISO load, graphics, physical devices, audible audio or frame pacing.')
if __name__=='__main__':main()
