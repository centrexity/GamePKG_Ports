#!/usr/bin/env python3
"""Maintainer-only reader/admission tests; users need no Python or extracted media."""
import argparse,hashlib,importlib.util,json,os,struct,subprocess,tarfile,tempfile
from pathlib import Path
BASES=[0,0xfd90,0x2080,0x30600,0xfd90000,0x18300000,0x1fb20000]
def put32(b,at,value):struct.pack_into('<I',b,at,value)
def node(b,at,name,sector,size,attribute=0,left=0,right=0):
    struct.pack_into('<HHIIBB',b,at,left,right,sector,size,attribute,len(name));b[at+14:at+14+len(name)]=name

def fixture(path,base=0,large=False):
    descriptor=bytearray(2048);descriptor[:20]=b'MICROSOFT*XBOX*MEDIA';descriptor[0x7ec:]=descriptor[:20];put32(descriptor,0x14,40);put32(descriptor,0x18,2048)
    root=bytearray(2048);start=2097153 if large else 60
    node(root,0,b'default.xbe',start,128,left=8,right=16)
    node(root,32,b'Data',41,2048,16)
    node(root,64,b'caf\xe9.bin',61,67)
    sub=bytearray(2048);node(sub,0,b'Thing.bin',70,4096)
    with path.open('w+b') as f:
        f.truncate(base+((start+2)*2048 if large else 80*2048))
        for off,data in [(65536,descriptor),(40*2048,root),(41*2048,sub),(start*2048,b'XBEH'+bytes(range(124))),(61*2048,bytes(range(67))),(70*2048,bytes((i*37)&255 for i in range(4096)))]:f.seek(base+off);f.write(data)
    return root

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--app',type=Path,required=True);parser.add_argument('--app-lib',type=Path,required=True);parser.add_argument('--iso',type=Path);parser.add_argument('--wine',action='store_true')
    args=parser.parse_args();here=Path(__file__).resolve().parent;lib=args.app_lib.resolve(strict=True)
    pin=json.loads((here/'reference_inputs.json').read_text());assert pin['production_ready'] is False
    kit=here.parents[3]/'takedawn/assets/takedawn-source-kit.tar.gz'
    assert hashlib.sha256(kit.read_bytes()).hexdigest()==pin['source_kit_sha256']
    with tarfile.open(kit) as archive:upstream=archive.extractfile(pin['upstream_reader_member']).read()
    assert hashlib.sha256(upstream).hexdigest()==pin['upstream_reader_sha256']
    assert hashlib.sha256((args.app/pin['reference_reader']).read_bytes()).hexdigest()==pin['reference_reader_sha256']
    spec=importlib.util.spec_from_file_location('save_checks',here.parent/'service-save/check.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
    with tempfile.TemporaryDirectory(prefix='gamepkg-disc-services-') as temp:
        root=Path(temp);exe=root/'admission'
        cmd=['rustc','--edition=2021','-D','warnings',str(here/'admission.rs'),'--extern',f'gamepkg={lib}','-L',f'dependency={lib.parent}','-o',str(exe)]
        for name in ['tempfile','toml']:cmd+=['--extern',f'{name}={helpers.dependency(lib,name)}']
        subprocess.run(cmd,check=True)
        for triple in ['x86_64-unknown-linux-gnu','i686-unknown-linux-gnu','x86_64-pc-windows-gnu','i686-pc-windows-gnu']:
            unit=root/('unit-'+triple+('.exe' if 'windows' in triple else ''))
            command=['rustc','--edition=2021','-D','warnings','--test','-O','--target',triple,str(here/'reference.rs'),'-o',str(unit)]
            if triple=='i686-unknown-linux-gnu':command+=['-C','linker=/usr/bin/gcc-11']
            subprocess.run(command,check=True)
            if 'windows' not in triple:subprocess.run([str(unit),'--test-threads=1'],check=True)
            elif args.wine:subprocess.run(['wine',str(unit),'--test-threads=1'],check=True,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'),timeout=90)
            else:print(f'PASS {triple}: compile only, runtime not exercised',flush=True)
        images=[]
        for base in BASES:
            image=root/f'base-{base:08x}.iso';fixture(image,base);images.append(image)
        image=root/'large-offset.iso';fixture(image,large=True);images.append(image)
        malformed=[('no-magic',65536,b'NO'),('bad-tail',65536+0x7ec,b'NO'),('root-size',65536+0x18,struct.pack('<I',1048577)),('root-outside',65536+0x14,struct.pack('<I',0xffffffff)),('bad-link',40*2048+2,struct.pack('<H',65534)),('overlap',40*2048+2,struct.pack('<H',1)),('duplicate-link',40*2048+2,struct.pack('<H',8)),('bad-dot',40*2048+13,b'\x02..'),('bad-slash',40*2048+14,b'a/b'),('bad-backslash',40*2048+14,b'a\\b'),('bad-colon',40*2048+14,b'a:b'),('bad-control',40*2048+14,b'\x1f'),('empty-name',40*2048+13,b'\0'),('bad-extent',40*2048+4,struct.pack('<I',0xffffffff)),('dir-cycle',40*2048+32+4,struct.pack('<I',40)),('same-path',40*2048+64+13,b'\x0bDEFAULT.XBE')]
        for name,offset,data in malformed:
            image=root/(name+'.iso');fixture(image)
            with image.open('r+b') as f:f.seek(offset);f.write(data)
            images.append(image)
        command=[str(exe),str(args.app.resolve()),str(here),*map(str,images)]
        if args.iso:command+=['--private-iso',str(args.iso.resolve(strict=True))]
        subprocess.run(command,check=True)
    print('Reader component checks only; no playable title or new admission permission.')
if __name__=='__main__':main()
