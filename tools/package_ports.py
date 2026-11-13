"""Maintainer-only release packaging. Players use the catalog in GamePKG."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,tomllib,zipfile
ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--cli',type=Path,required=True);p.add_argument('--key',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
output=args.output.resolve();output.mkdir(parents=True,exist_ok=True)
assert args.key.is_file(),'Supply the existing private creator key; do not generate a replacement for updates'
assert not args.key.resolve().is_relative_to(output),'Keep the private key outside release output'
cli=args.cli.resolve();catalog=tomllib.loads((ROOT/'ports.gamepkglist').read_text());public=None
config=output/'verification-settings.toml';config.write_text('automatic_update_checks=false\n')
env=dict(os.environ,GAMEPKG_HOME=str(output/'verification-library'),GAMEPKG_CONFIG=str(config))
def run(*a):
 r=subprocess.run([str(cli),*map(str,a)],env=env,text=True,capture_output=True,timeout=180)
 assert r.returncode==0,r.stderr
 return r.stdout
assets=[]
for game in catalog['games']:
 name=game['id'].removeprefix('com.gamepkg.');source=ROOT/name;manifest=tomllib.loads((source/'manifest.toml').read_text())
 assert manifest['id']==game['id'] and manifest['version']==game['version']
 assert bool(manifest.get('requires_rom') or manifest.get('roms'))==game['requires_rom']
 assert game['download_url'].endswith('/'+name+'.gamepkg')
 archive=output/(name+'.gamepkg');assert not archive.exists(),'Use a fresh output folder to avoid replacing a signed release'
 run('pack',source,'-o',archive,'--key',args.key.resolve());run('verify',archive)
 with zipfile.ZipFile(archive) as z:
  assert not any(n.lower().endswith(('.wad','.sfc','.smc','.rom','.key','.iso','.xiso','.map')) for n in z.namelist())
  signed=tomllib.loads(z.read('manifest.toml').decode());cargo=tomllib.loads(z.read('source/Cargo.toml').decode())
  assert not cargo.get('dependencies')
  if public is None:public=signed['creator_key']
  assert signed['creator_key']==public
 assert manifest['update_url']=='https://github.com/centrexity/GamePKG_Ports/releases/latest/download/'+archive.name
 assets.append(archive)
identity={'creator_public_key':public,'creator_fingerprint':hashlib.sha256(bytes.fromhex(public)).hexdigest()}
known=ROOT/'creator-identity.json'
if known.exists():assert json.loads(known.read_text())==identity,'Creator key changed; updates must retain the established key'
(output/'ports.gamepkglist').write_bytes((ROOT/'ports.gamepkglist').read_bytes());run('import-list',output/'ports.gamepkglist')
(output/'creator-identity.json').write_text(json.dumps(identity,indent=2)+'\n')
assets.extend([output/'ports.gamepkglist',output/'creator-identity.json'])
(output/'ports.sha256').write_text(''.join(hashlib.sha256(a.read_bytes()).hexdigest()+'  '+a.name+'\n' for a in assets))
print(json.dumps({'packages':len(catalog['games']),'output':str(output),'creator_fingerprint':identity['creator_fingerprint'],'release_assets':[a.name for a in assets]+['ports.sha256']}))
