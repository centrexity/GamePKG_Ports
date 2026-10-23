"""Check the published catalog recipe metadata without downloading any game."""
from pathlib import Path
import hashlib,json,tomllib,re
from urllib.parse import urlsplit
ROOT=Path(__file__).resolve().parents[1]
identity=json.loads((ROOT/'creator-identity.json').read_text());public=bytes.fromhex(identity['creator_public_key'])
assert len(public)==32 and hashlib.sha256(public).hexdigest()==identity['creator_fingerprint']
halo=ROOT/'halo'
assert (halo/'manifest.toml').is_file() and not (halo/'manifest.draft.toml').exists()
assert 'com.gamepkg.halo' not in (ROOT/'ports.gamepkglist').read_text()
pin=json.loads((ROOT/'tools'/'opence-pin.json').read_text())
assert pin['vendored'] is False and pin['license']=='AGPL-3.0-or-later'
assert pin['commit']=='ce95f7deed5bbf726e50a9d5fe5227437194cf3b' and pin['profile']=='opence-ce95f7de'
manifest=(halo/'manifest.toml').read_text()
assert 'builtin_engine = "opence-ce95f7de"' in manifest and 'id = "com.gamepkg.halo"' in manifest
assert pin['commit'][:8] in manifest and 'network = false' in manifest
import os
forbidden=[]
for dirpath, dirnames, filenames in os.walk(halo):
    dirnames[:]=[name for name in dirnames if name!='.git']
    for name in dirnames+filenames:
        lowered=name.casefold()
        if lowered in {'maps','custom_maps'} or lowered.endswith(('.iso','.xiso','.map','.wad','.sfc','.smc','.rom')):
            forbidden.append(name)
assert not forbidden,forbidden
print('OpenCE recipe is unvendored, unpublished, and contains no game data.')
catalog=tomllib.loads((ROOT/'ports.gamepkglist').read_text());assert catalog['format_version']==1
assert {g['id'] for g in catalog['games']}=={'com.gamepkg.'+n for n in ['2048','arkanoo','doom','zelda3','cube2']}
for game in catalog['games']:
 name=game['id'].removeprefix('com.gamepkg.');source=ROOT/name;m=tomllib.loads((source/'manifest.toml').read_text())
 assert game['version']==m['version'] and game['id']==m['id']
 # Catalog previews may explain host/data requirements beyond signed descriptions.
 assert m.get('description')
 assert isinstance(game.get('description'),str) and 0<len(game['description'])<=2048 and not any(ord(c)<32 for c in game['description'])
 assert m.get('title') and m.get('author') and m.get('homepage') and m.get('genres')
 assert game['requires_rom']==bool(m.get('requires_rom') or m.get('roms'))
 # A release can add a package without changing its game version, and retain
 # immutable URLs for older unchanged packages.
 url=urlsplit(game['download_url'])
 assert url.scheme=='https' and url.netloc=='github.com' and not url.query and not url.fragment
 assert re.fullmatch(r'/centrexity/GamePKG_Ports/releases/download/v[0-9]+\.[0-9]+\.[0-9]+/'+re.escape(name)+r'\.gamepkg',url.path)
 assert m['update_url']==f'https://github.com/centrexity/GamePKG_Ports/releases/latest/download/{name}.gamepkg'
 assert hashlib.sha256((source/'source/Cargo.lock').read_bytes()).hexdigest()==m['lockfile_sha256']
 assert not tomllib.loads((source/'source/Cargo.toml').read_text()).get('dependencies')
print('All five catalog entries match their dependency-free recipes and release/update URLs.')

# Artwork provenance and signed attribution travel with each thumbnail update.
import struct
provenance=json.loads((ROOT/'artwork/thumbnails/provenance.json').read_text())
assert {item['game'] for item in provenance}=={game['id'].removeprefix('com.gamepkg.') for game in catalog['games']}
for item in provenance:
 data=(ROOT/item['game']/'thumbnail.png').read_bytes()
 assert data[:8]==b'\x89PNG\r\n\x1a\n'
 width,height=struct.unpack('>II',data[16:24]);assert 16<=width<=512 and 16<=height<=512
 assert hashlib.sha256(data).hexdigest()==item['thumbnail_sha256']
 notice=(ROOT/item['game']/'NOTICE').read_text()
 assert item['author'] in notice and item['license'].split('-')[0] in notice
print('All five thumbnails have bounded PNG dimensions, matching hashes and author/license notices.')
