# DOOM for GamePKG

Requires a GamePKG host with the `doom-e257a095` builtin engine (current main), the isolated Linux compiler, and your own original PC DOOM.WAD or DOOM2.WAD. The older 0.1.6 release binaries do not include this engine. No WAD or extracted assets are included. The host reads your selected file in place into a read-only session snapshot; it never rewrites it.

The recipe has no Cargo dependencies. `gamepkg::doom_tick` calls a pinned host-owned engine compiled locally into Wasm with zero host imports, a 64 MiB engine cap, and time deadlines. Total package memory is capped at 96 MiB. Arbitrary packages cannot supply C sources or replacement engines.

```sh
gamepkg keygen -o /path/to/private/creator.key
gamepkg pack /path/to/GamePKG_Ports/doom -o /path/to/doom.gamepkg --key /path/to/private/creator.key
gamepkg install /path/to/doom.gamepkg --trust-creator
gamepkg build com.gamepkg.doom
gamepkg run com.gamepkg.doom --rom /path/to/DOOM.WAD --frames 120
gamepkg-gui
```

Select the package and add your WAD under **ROM files**, then Play. No signing key or Python is needed for players.

Arrows/WASD move forward/back and turn. Z fires, C uses doors/switches, X runs, V toggles the automap, Q/E strafe, Enter confirms menus, and Tab opens/closes the menu. Controller/touch B fires, A uses, Y runs, X maps, shoulders strafe, Start confirms and Select opens the menu. Hold both shoulders and press Start (Q+E+Enter) to quicksave slot 0. A saved game resumes on the next launch. Only one persistent slot is supported.

The port provides original 320×200 rendering, single-player gameplay, and mixed WAD sound effects at 48 kHz mono. Music synthesis, mouse look and multiplayer are not supplied. Use menus to start a new game or choose another level of difficulty. WADs containing mods or other games are not supported by this recipe.

Engine source and setup: https://github.com/centrexity/GamePKG/tree/main/engines/doom
