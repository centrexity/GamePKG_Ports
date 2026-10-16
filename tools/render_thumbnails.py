"""Maintainer-only thumbnail rendering; players need no artwork tools or Python."""
from pathlib import Path
import hashlib, json, shutil, subprocess, xml.etree.ElementTree as ET
ROOT=Path(__file__).resolve().parents[1];ART=ROOT/'artwork/thumbnails';SOURCES=ART/'sources'
items=[('doom','skull-crack','Lorc','https://game-icons.net/1x1/lorc/skull-crack.html','#411a2c','#a52d42','#ffd6a2','DOOM','CLASSIC SHOOTER'),('zelda3','broadsword','Lorc','https://game-icons.net/1x1/lorc/broadsword.html','#092b33','#197368','#ffe09a','ZELDA3','ADVENTURE'),('arkanoo','brick-wall','Delapouite','https://game-icons.net/1x1/delapouite/brick-wall.html','#172653','#6151ad','#9beaf1','ARKANOO','BRICK BREAKER')]
records=[]
for name,icon,author,url,bg,bright,ink,title,genre in items:
 tree=ET.fromstring((SOURCES/(icon+'.svg')).read_text())
 paths=''.join('<path fill="'+ink+'" d="'+node.attrib['d']+'"/>' for node in tree.iter() if node.tag.endswith('}path'))
 assert paths
 svg=f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512"><defs><linearGradient id="bg" x2="1" y2="1"><stop stop-color="{bright}"/><stop offset="1" stop-color="{bg}"/></linearGradient></defs><rect width="512" height="512" rx="40" fill="url(#bg)"/><circle cx="430" cy="58" r="210" fill="white" opacity=".04"/><path d="M-30 350L545 64M-30 405L545 119" stroke="white" stroke-width="1" opacity=".08"/><g transform="translate(116 43) scale(.55)">{paths}</g><text x="256" y="399" fill="white" font-family="DejaVu Sans" font-size="53" font-weight="bold" text-anchor="middle">{title}</text><text x="256" y="444" fill="{ink}" font-family="DejaVu Sans" font-size="19" letter-spacing="4" text-anchor="middle">{genre}</text></svg>'''
 (ART/(name+'.svg')).write_text(svg)
 subprocess.run(['convert','-background','none','-font','DejaVu-Sans',str(ART/(name+'.svg')), '-define','png:color-type=6',str(ROOT/name/'thumbnail.png')],check=True)
 notice=f'''\nThumbnail artwork: "{icon.replace('-',' ').title()}" by {author}, via Game-icons.net.\nSource: {url}\nLicense: Creative Commons Attribution 3.0 Unported (CC BY 3.0)\nhttps://creativecommons.org/licenses/by/3.0/\nChanges: recolored and resized icon; original gradient, lettering and composition by GamePKG contributors. This is a generic illustration, not original game artwork.\n\n'''+(SOURCES/'CC-BY-3.0.txt').read_text()
 p=ROOT/name/'NOTICE';base=p.read_text().split('\nThumbnail artwork:')[0];base=base.replace('GamePKG icon: Copyright 2026 GamePKG contributors, Apache-2.0.\n','');p.write_text(base.rstrip()+'\n'+notice)
 records.append(dict(game=name,source=url,author=author,license='CC-BY-3.0',source_sha256=hashlib.sha256((SOURCES/(icon+'.svg')).read_bytes()).hexdigest(),thumbnail_sha256=hashlib.sha256((ROOT/name/'thumbnail.png').read_bytes()).hexdigest()))
shutil.copyfile(SOURCES/'2048.png',ROOT/'2048/thumbnail.png')
p=ROOT/'2048/NOTICE';base=p.read_text().split('\nThumbnail artwork:')[0];p.write_text(base.rstrip()+'\n\nThumbnail artwork: original 2048 apple-touch-icon.png by Gabriele Cirulli.\nSource: https://github.com/gabrielecirulli/2048/blob/'+(SOURCES/'2048-upstream-commit.txt').read_text().strip()+'/meta/apple-touch-icon.png\nLicense: MIT. Copied unchanged.\n\n'+(SOURCES/'2048-LICENSE.txt').read_text())
records.append(dict(game='2048',source='https://github.com/gabrielecirulli/2048',author='Gabriele Cirulli',license='MIT',source_revision=(SOURCES/'2048-upstream-commit.txt').read_text().strip(),thumbnail_sha256=hashlib.sha256((ROOT/'2048/thumbnail.png').read_bytes()).hexdigest()))
(ART/'provenance.json').write_text(json.dumps(records,indent=2)+'\n')
print('Rendered three attributed CC BY 3.0 covers and copied the original MIT 2048 icon.')
