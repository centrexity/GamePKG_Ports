"""Check the published catalog recipe metadata without downloading any game."""
from pathlib import Path
import hashlib,json,tomllib
ROOT=Path(__file__).resolve().parents[1]
identity=json.loads((ROOT/'creator-identity.json').read_text());public=bytes.fromhex(identity['creator_public_key'])
assert len(public)==32 and hashlib.sha256(public).hexdigest()==identity['creator_fingerprint']
catalog=tomllib.loads((ROOT/'ports.gamepkglist').read_text());assert catalog['format_version']==1
assert {g['id'] for g in catalog['games']}=={'com.gamepkg.'+n for n in ['2048','arkanoo','doom','zelda3']}
for game in catalog['games']:
 name=game['id'].removeprefix('com.gamepkg.');source=ROOT/name;m=tomllib.loads((source/'manifest.toml').read_text())
 assert game['version']==m['version'] and game['id']==m['id']
 assert game.get('description')==m.get('description') and m.get('description')
 assert m.get('title') and m.get('author') and m.get('homepage') and m.get('genres')
 assert game['requires_rom']==bool(m.get('requires_rom') or m.get('roms'))
 assert game['download_url']==f"https://github.com/centrexity/GamePKG_Ports/releases/download/v{m['version']}/{name}.gamepkg"
 assert m['update_url']==f'https://github.com/centrexity/GamePKG_Ports/releases/latest/download/{name}.gamepkg'
 assert hashlib.sha256((source/'source/Cargo.lock').read_bytes()).hexdigest()==m['lockfile_sha256']
 assert not tomllib.loads((source/'source/Cargo.toml').read_text()).get('dependencies')
print('All four catalog entries match their dependency-free recipes and release/update URLs.')

# Artwork provenance and signed attribution travel with each thumbnail update.
import struct
for item in json.loads((ROOT/'artwork/thumbnails/provenance.json').read_text()):
 data=(ROOT/item['game']/'thumbnail.png').read_bytes()
 assert data[:8]==b'\x89PNG\r\n\x1a\n'
 width,height=struct.unpack('>II',data[16:24]);assert 16<=width<=512 and 16<=height<=512
 assert hashlib.sha256(data).hexdigest()==item['thumbnail_sha256']
 notice=(ROOT/item['game']/'NOTICE').read_text()
 assert item['author'] in notice and item['license'].split('-')[0] in notice
print('All four thumbnails have bounded PNG dimensions, matching hashes and author/license notices.')
