# Cube 2: Sauerbraten

This dependency-free signed selector uses the original engine source pinned in [GamePKG](https://github.com/centrexity/GamePKG/tree/main/engines/cube2), revision 6889. It contains no original game media. GamePKG builds the reviewed engine locally; the package cannot provide a replacement engine or dependencies.

Use **GamePKG 0.1.15 on Linux**. Install GCC/G++, Make, SDL2, SDL2_image, SDL2_mixer, zlib and OpenGL development libraries. See [Linux installation](https://github.com/centrexity/GamePKG/blob/main/docs/INSTALL.md). Python is not required. This engine is not supported by the Windows player yet.

Import this repository’s catalog, choose Cube 2, download/inspect and approve its creator, then Build. On its game page, select your original Sauerbraten folder containing `data` and `packages`. The engine reads these folders in place, read only. Set Settings → Memory ceiling to at least 2048 MiB. Software rendering may need a 500 ms tick deadline. Press Play to open the separate game-only window, presented through Vulkan by default.

Original keyboard, console, mouse and editing controls are preserved. Click the game to capture the mouse; Escape releases capture. Close the window or use the game’s Quit command to save preferences and edited maps privately. The library remembers the data folder and tracks play time, including while minimized.

Multiplayer stays offline until you choose a direct server, local hosting address, or optional trusted master in the library. The master choice permits browsing and joining only its validated IPv4 server list. Local/private addresses require their separate opt-in. Hosting is not publicly advertised. The host refreshes master data asynchronously; if the first browser request precedes retrieval, refresh again after a few seconds. **Use offline play** clears all these choices for the next launch.

See the [engine guide](https://github.com/centrexity/GamePKG/blob/main/engines/cube2/README.md) for limits, saves, networking and verification. Original media licenses remain with their owners. The thumbnail is an original geometric illustration; it includes no game artwork or logo.
