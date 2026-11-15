#!/usr/bin/env python3
"""Maintainer-only pinned target classifier generation and exhaustive table checks."""
import argparse, hashlib, json, os, struct, subprocess, tarfile, tempfile, shutil
from pathlib import Path
KIT='6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4'
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--wine',action='store_true')
    args=parser.parse_args();here=Path(__file__).resolve().parent;target=args.target_dir.resolve();output=args.output.resolve()
    assert not output.exists()
    kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
    assert hashlib.sha256(kit.read_bytes()).hexdigest()==KIT
    env=dict(os.environ,CARGO_TARGET_DIR=str(target))
    subprocess.run(['cargo','test','--offline','--locked','--manifest-path',str(here/'maintainer/Cargo.toml')],check=True,env=env)
    subprocess.run(['cargo','build','--offline','--locked','--manifest-path',str(here/'maintainer/Cargo.toml'),'--bin','takedawn_call_targets'],check=True,env=env)
    tool=target/'debug/takedawn_call_targets'
    names=['prepared/src/recomp_dispatch.rs','prepared/src/recomp_manual.rs','prepared/src/recomp_stubs_unresolved.rs','game/src/game/soundtrack.c','game/src/game/soundtrack.h','toolkit/src/kernel/kernel_bridge.c','toolkit/src/kernel/kernel.h']
    with tempfile.TemporaryDirectory(prefix='gamepkg-call-targets-') as name:
        scratch=Path(name);inputs=scratch/'inputs';inputs.mkdir()
        with tarfile.open(kit) as archive:
            for filename in names:
                member=archive.getmember(filename);assert member.isfile() and member.size<16*1024*1024
                (inputs/Path(filename).name).write_bytes(archive.extractfile(member).read())
        dispatch=inputs/'recomp_dispatch.rs';original=dispatch.read_bytes();dispatch.write_bytes(original+b'\n')
        negative=scratch/'must-not-exist'
        assert subprocess.run([str(tool),str(inputs),str(negative)],capture_output=True).returncode!=0 and not negative.exists()
        dispatch.write_bytes(original)
        subprocess.run([str(tool),str(inputs),str(output)],check=True)
        report=json.loads((output/'call_targets.json').read_text())
        assert report['production_ready'] is False and len(report['rows'])==24001
        assert report['tool_sha256']==hashlib.sha256((here/'maintainer/src/bin/takedawn_call_targets.rs').read_bytes()).hexdigest()
        for filename,expected in report['emitted_files'].items():assert hashlib.sha256((output/filename).read_bytes()).hexdigest()==expected
        manual=set(report['manual_wrappers']);stubs=set(report['unresolved_stubs'])
        known={address:((6 if symbol in stubs else 2 if symbol in manual else 3 if symbol=='xbe_entry_point' else 1)<<24)|index for index,(address,symbol) in enumerate(report['rows'])}
        def expected(address):
            if 0xfe7f5700<=address<0xfe7f5728 and address%4==0:return 0x04000000|(address-0xfe7f5700)//4
            if address in known:return known[address]
            if 0xfe000000<=address<0xfe0005e8:return 0x05000000|(address-0xfe000000)//4
            return 0
        addresses={0,1,0xffffffff,0x1d2807}
        for address in known:addresses.update([address-1,address,address+1])
        addresses.update(range(0xfe000000-4,0xfe0005e8+4));addresses.update(range(0xfe7f5700-4,0xfe7f5728+4))
        seed=0xa71946ce
        for _ in range(32768):
            seed^=seed<<13;seed&=0xffffffff;seed^=seed>>17;seed^=seed<<5;seed&=0xffffffff;addresses.add(seed)
        vectors=output/'lookup-vectors'
        with vectors.open('wb') as stream:
            stream.write(struct.pack('<I',len(addresses)))
            for address in sorted(addresses):stream.write(struct.pack('<II',address,expected(address)))
        for filename in report['emitted_files']:
            destination=scratch/filename;destination.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(output/filename,destination)
        wrapper=scratch/'lookup.rs'
        wrapper.write_text('mod targets;fn main(){let bytes=std::fs::read(std::env::args().nth(1).unwrap()).unwrap();let count=u32::from_le_bytes(bytes[..4].try_into().unwrap());assert_eq!(bytes.len(),4+count as usize*8);for row in bytes[4..].chunks_exact(8){let address=u32::from_le_bytes(row[..4].try_into().unwrap());let expected=u32::from_le_bytes(row[4..].try_into().unwrap());assert_eq!(targets::original_target(address),expected,"target {address:08x}");}println!("PASS {count} original-target membership, entrypoint/manual identity, miss and kernel/soundtrack boundary cases; handlers never execute");}')
        for triple in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
            exe=scratch/('targets-'+triple+('.exe' if 'windows' in triple else ''))
            command=['rustc','--edition=2021','-D','warnings','-O','--target',triple,str(wrapper),'-o',str(exe)]
            if triple=='i686-unknown-linux-gnu':command+=['-C','linker=/usr/bin/gcc-11']
            subprocess.run(command,check=True)
            if 'windows' not in triple:subprocess.run([str(exe),str(vectors)],check=True)
            elif args.wine:subprocess.run(['wine',str(exe),str(vectors)],check=True,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'),timeout=60)
        summary={k:report[k] for k in ['schema','production_ready','source_kit_sha256','tool_sha256','lock_sha256','inputs','manual_wrappers','unresolved_stubs','kernel_slots','soundtrack_slots','qualification']}
        summary.update(target_rows=len(known),lookup_cases_per_target=len(addresses),emitted_source_files=len(report['emitted_files']),emitted_inventory_sha256=hashlib.sha256(json.dumps(report['emitted_files'],sort_keys=True,separators=(',',':')).encode()).hexdigest())
        (output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
        print('Source classifier evidence only; no resolver execution, kernel/manual implementation or gameplay qualification.')
if __name__=='__main__':main()
