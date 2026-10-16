# GamePKG Ports

Source recipes for games adapted to [GamePKG](https://github.com/centrexity/GamePKG), maintained by Centrexity.

| Port | Recipe | Requirements |
| --- | --- | --- |
| DOOM | [doom](doom) | Current GamePKG main with the pinned DOOM engine and your own DOOM.WAD/DOOM2.WAD |
| Zelda3 | [zelda3](zelda3) | GamePKG 0.1.6 or newer and your own Zelda: A Link to the Past (USA) ROM |
| Arkanoo | [arkanoo](arkanoo) | GamePKG |
| 2048 | [2048](2048) | GamePKG |

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
