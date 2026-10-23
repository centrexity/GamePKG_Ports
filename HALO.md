# Halo: Combat Evolved (OpenCE)

Linux port of [OpenCE](https://github.com/OpenCommunityEdition/OpenCE) commit `ce95f7deed5bbf726e50a9d5fe5227437194cf3b`. The recipe is not in the published catalog yet. Pack and install it from this checkout. GamePKG 0.1.17 or newer on Linux is required.

No Halo maps, disc images, or cache files are in this repository. OpenCE is AGPL-3.0-or-later and is not vendored here. The host clones that exact commit into your library, checks the license digest, and builds `build/linux/halo`. Play runs that 32-bit program in its own window.

## What you supply

A folder that contains either:

- `maps/ui.map` extracted from a Halo: Combat Evolved Xbox disc you own, or
- exactly one `.iso` or `.xiso` of that disc.

Custom Edition caches stay in a `custom_maps` folder you keep beside those maps. Do not put any of those files in this repository.

## Build tools

The portable Linux build needs Python 3, Ninja, Clang, CMake, pkg-config, wayland-scanner, and 32-bit glibc headers (`gcc-multilib`, `libc6-dev-i386`). It downloads a Debian 11 sysroot and SDL3 on the first compile. Ubuntu 24.04 has no SDL3 package; the portable build is what the host runs. Bubblewrap, when installed, starts the game without a network. 32-bit ALSA (`libasound.so.2` for i386) is needed for sound; otherwise the game starts with SDL's dummy audio driver.

## Play

```sh
gamepkg pack halo -o /tmp/halo.gamepkg --key /path/to/creator.key
gamepkg install /tmp/halo.gamepkg --trust-creator
gamepkg build com.gamepkg.halo
gamepkg run com.gamepkg.halo --game-data /path/to/your/halo-folder
```

Or use Build and Play in the desktop library, then choose the folder. Set the memory ceiling to at least 2048 MiB. The engine opens its own window. Saves, config, and a disc extraction stay under the library's runtime directory, not beside your disc. An extracted `maps/` folder is linked read-only from that runtime directory so OpenCE's `debug.txt` is not written into your folder. Automatic OpenCE updates are turned off.

`HALO_HIDDEN_WINDOW`, `HALO_NULL_RENDERER`, and `HALO_EXIT_AFTER` are passed through when you set them. They are OpenCE's own debug switches. Leave them unset for normal play.

`GAMEPKG_OPENCE_SOURCE` can point at a checkout of the pinned commit that you already built. The host still checks the commit and license digest.

This is not the Cube 2 frame bridge. There is no Wasm build, no Windows player, and no catalog download until a maintainer publishes a release. See the host notes in GamePKG `engines/halo/README.md`.
