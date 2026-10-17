# DOOM for GamePKG

Requires a GamePKG host with the `doom-e257a095` builtin engine (current main), the isolated Linux compiler, and your own original PC DOOM.WAD or DOOM2.WAD. Use the updated source host for the features described below; older release builds provide a subset. No WAD or extracted assets are included. The host reads your selected file in place into a read-only session snapshot; it never rewrites it.

The recipe has no Cargo dependencies. `gamepkg::doom_tick` calls a pinned host-owned engine compiled locally for an isolated native Linux worker by default, with a separate optional Wasm build. The Wasm engine has zero host imports and a 64 MiB engine cap. Both targets retain time and memory limits. Total package memory is capped at 96 MiB. Arbitrary packages cannot supply C sources or replacement engines.

```sh
gamepkg keygen -o /path/to/private/creator.key
gamepkg pack /path/to/GamePKG_Ports/doom -o /path/to/doom.gamepkg --key /path/to/private/creator.key
gamepkg install /path/to/doom.gamepkg --trust-creator
gamepkg build com.gamepkg.doom
gamepkg run com.gamepkg.doom --rom /path/to/DOOM.WAD --frames 120
gamepkg-gui
```

Select the package and add your WAD under **ROM files**, then Play. No signing key or Python is needed for players.

With the updated source host, desktop controls use original bindings: arrows move
and turn, Ctrl fires, Space uses doors/switches, Shift runs, Alt strafes, comma
and period strafe left/right, and Tab toggles the automap. Enter confirms menus,
Escape opens/closes them, and original function-key shortcuts work. Mouse buttons,
relative movement and configured wheel bindings go through the original engine.
Ctrl+Alt releases pointer capture; clicking the game recaptures it. Focus loss
releases held controls. Controller/touch B fires, A uses, Y runs, X maps, shoulders
strafe, Start confirms and Select opens the menu.

All six original save slots and both configuration files persist. Original save
and load menus select slots; a saved game resumes on launch. Controller users can
hold both shoulders and press Start to save slot 0. The updated host also provides
48 kHz stereo effects and original WAD music through its in-memory OPL backend.
Original quitting includes the quit sound and ENDOOM screen; press a new key or
mouse button to close it. Multiplayer, extra WAD/mod loading and music backend
selection remain unfinished. These additions require the updated source host;
older release binaries may provide fewer features.

Engine source and setup: https://github.com/centrexity/GamePKG/tree/main/engines/doom
