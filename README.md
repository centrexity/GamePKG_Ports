# GamePKG Ports

Source recipes for games adapted to [GamePKG](https://github.com/centrexity/GamePKG), maintained by Centrexity.

## Import into GamePKG

In the desktop Library source field paste:

```text
https://github.com/centrexity/GamePKG_Ports
```

Choose **Import .gamepkglist (no game downloads)**. The repository URL selects the catalog from the latest published stable release. Games appear as **Not downloaded**. Choose a game's **Download / inspect**, review the creator, install, then Build and Play. ROM/WAD-required recipes need your own selected files. Cube 2 needs GamePKG 0.1.15 on Linux and your original game-data installation.

You can also download [ports.gamepkglist](https://github.com/centrexity/GamePKG_Ports/releases/latest/download/ports.gamepkglist) and drag it into GamePKG. The [catalog source](ports.gamepkglist) pins each package to its published release. Importing a list downloads no game packages. Package signatures and creator continuity are verified when you download a game; catalog metadata alone is untrusted.

CLI:

```sh
gamepkg import-list https://github.com/centrexity/GamePKG_Ports
gamepkg list
gamepkg download com.gamepkg.2048 --trust-creator
gamepkg build com.gamepkg.2048
```

Review the [published creator fingerprint](creator-identity.json) before approving first installations. Players never enter a private key. Release [checksums](https://github.com/centrexity/GamePKG_Ports/releases/latest/download/ports.sha256) detect changed downloads. Packages contain source recipes and assets owned by their respective authors; ROMs, WADs, extracted commercial assets and signing keys are excluded.

## Available ports

| Port | Recipe | Requirements |
| --- | --- | --- |
| Cube 2: Sauerbraten | [cube2](cube2) | GamePKG 0.1.15 on Linux and an original Sauerbraten installation; see [setup](CUBE2.md) |
| DOOM | [doom](doom) | Current GamePKG main with the pinned DOOM engine and your own DOOM.WAD/DOOM2.WAD |
| Zelda3 | [zelda3](zelda3) | GamePKG 0.1.6 or newer and your own Zelda: A Link to the Past (USA) ROM |
| Arkanoo | [arkanoo](arkanoo) | GamePKG |
| 2048 | [2048](2048) | GamePKG |
| Halo: Combat Evolved | [halo](halo) | GamePKG 0.1.17 or newer, 32-bit Linux (i686). 32-bit Windows can be cross-compiled. You supply the Xbox disc or extracted maps. The package has no Halo assets and no OpenCE source. See [HALO.md](HALO.md). |

Use the GamePKG CLI to create a package with your creator key:

```sh
gamepkg keygen --output creator.key
gamepkg pack arkanoo --output arkanoo.gamepkg --key creator.key
gamepkg install arkanoo.gamepkg --trust-creator
gamepkg build com.gamepkg.arkanoo
gamepkg-gui --play com.gamepkg.arkanoo
```

Players do not need signing keys or Python. Zelda uses a registered ROM in place; this repository contains no ROM or extracted ROM assets. See the [Zelda setup guide](https://github.com/centrexity/GamePKG/blob/main/engines/zelda3/README.md).

GamePKG pins this repository as its `ports` Git submodule. Host integration tests exercise these recipes, including gameplay rules, source validation, isolated builds, GUI play, and compiler exchanges. Clone the host with `git clone --recurse-submodules https://github.com/centrexity/GamePKG.git`; update an existing checkout with `git submodule update --init --recursive`.

See [DOOM setup and controls](DOOM.md) for the new source recipe.

The trusted DOOM and Zelda engines and runtime security controls remain in GamePKG. Existing official signed package download and update endpoints remain available in the host repository.

## Provenance and licensing

These recipes were moved without source changes from Centrexity/GamePKG commit `1ed7ffd11a6237db288c08fd334d74f07ebd6a07`: `examples/ports/zelda3`, `examples/good/arkanoo`, and `examples/good/2048`. Each port retains its upstream LICENSE, NOTICE, and CREDITS documents. Upstream code remains under those licenses; Centrexity additions are Apache-2.0 unless a file states otherwise.

## Maintainer releases

Use the existing private release creator key, kept outside this repository and release output. Bump changed package versions, retain IDs/creator identity/save schema as appropriate, set each stable update URL, and update the catalog to immutable versioned release asset URLs. Pack and verify with a current GamePKG CLI:

```sh
python3 tools/package_ports.py --cli /path/to/gamepkg --key /private/release.key --output /path/to/fresh/release-output
```

This is a maintainer tool; players need no Python. It checks catalog/package metadata, creator identity, signatures, dependency-free source and excluded file types, then writes the signed packages listed in the catalog, the catalog, the public identity and checksums. Upload only the explicit `release_assets` it reports, never the verification folder or private key. Verify builds/playback locally, publish the tagged release, then test repository-URL import and each downloaded signature in a fresh library. Never replace assets in an already published release; create a higher version for changes.

Pushing a `vX.Y.Z` tag runs `.github/workflows/release.yml`. That job builds the GamePKG `main` pack CLI, signs every catalog package with the `GAMEPKG_PORTS_RELEASE_KEY` secret, and uploads the assets. The secret must be the existing creator seed whose public key is in `creator-identity.json`. GamePKG `main` must already contain the external-engine catalog before the Halo package can be signed. The catalog download URLs for this source tree point at `v0.1.6`. Releases `v0.1.4` and `v0.1.5` stay as they were published.

## Thumbnail artwork

2048 uses Gabriele Cirulli’s original MIT-licensed icon. The other covers adapt Game-icons.net icons by Lorc and Delapouite under CC BY 3.0. Their signed package NOTICE files include source links, author credits, changes and full license text. The covers are generic illustrations; no ROM-derived artwork is included. Original sources, checksums and editable compositions are in [artwork/thumbnails](artwork/thumbnails).

Version 0.1.4 packages and catalogs require GamePKG 0.1.13 or newer for signed titles, descriptions, authors, project links and genres. Update the host before importing the new catalog. Catalog descriptions are unverified previews; installed game pages use the signed manifest metadata.
