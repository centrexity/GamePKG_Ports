# Halo: Combat Evolved (OpenCE)

The selector `com.gamepkg.halo` builds the reviewed [OpenCE](https://github.com/OpenCommunityEdition/OpenCE) commit `ce95f7deed5bbf726e50a9d5fe5227437194cf3b` with the GamePKG host. Linux defaults to the tested native `linux-i686` target on both 32-bit and 64-bit machines. It runs in its own game-only window through SDL3 and hardware OpenGL, without Wine, Proton, WSL or CPU emulation.

This repository contains no Halo maps, disc images, extracted caches or OpenCE source. OpenCE is AGPL-3.0-or-later. The selector contains a dependency-free Rust launcher; GamePKG verifies the pinned engine source, license and reviewed patches before building it.

## Your game data

Choose your own Xbox `.iso` or `.xiso` directly in the game page's file picker. Other games can remain beside it. GamePKG remembers that selection per game and creator, and the engine reads the original image in place through a bounded, read-only view. Existing folders containing `maps/ui.map` also work. Keep original game data out of this repository.

The engine's decompressed map cache uses disposable RAM. It is separate from persistent saves and disappears on exit; maps and the disc image are not copied to disk. The reviewed maximum budget is 4 GiB: 3 GiB for the process address space and 1 GiB for that RAM cache. The recommended total is 3 GiB. Set the GamePKG memory ceiling to 4096 MiB for this profile.

## Build and play

With the matching reviewed host and an installed selector:

```sh
gamepkg build com.gamepkg.halo
gamepkg run com.gamepkg.halo --game-data /path/to/your/disc.xiso.iso
```

Subsequent launches can use the remembered selection. Build and Play are also available on the desktop game page. Normal launches require a working game window; graphics initialization failures report an error instead of silently running without a picture.

Linux needs Clang, LLVM tools, Ninja, CMake, pkg-config and xz, plus the host's 32-bit graphics/audio drivers. GamePKG's Rust preparation driver downloads and verifies the compiler sysroot and SDL sources, reconstructs reviewed generated resources, and builds the engine without running Python. Python is only a maintainer generation/test tool. Wayland also needs the 32-bit EGL loader; X11 uses the corresponding GLX drivers. Missing 32-bit ALSA is reported and falls back to silent audio.

`GAMEPKG_ENGINE_SOURCE` can select an existing checkout of the pinned source. Commit and license checks still apply. Unchanged builds reuse their outputs; compiler, CPU, preparation or reviewed-patch changes invalidate the relevant cache.

## Controllers and multiplayer

The engine uses SDL3's controller backend. A virtual Xbox controller has exercised buttons, movement, aiming, firing, rumble and disconnect/reconnect in native gameplay. Physical Bluetooth-controller qualification remains separate.

This selector requests networking. Linux keeps the read-only filesystem view, bounded RAM cache and memory cap when networking is enabled; an offline selector additionally receives a separate network namespace. Required isolation tools must be available. The upstream multiplayer tests exercise two native processes with movement, damage, deaths and synchronization checks; these are scoped regression checks, not a full campaign or internet-traversal qualification.

Original HUD pixels are upscaled locally. The reviewed SSE2 kernel preserves the original interpolation, centers, channel order, clamping and rounding, with a scalar fallback. Asset-free tests compare complete pixel buffers and floating-point flags across rounding modes.

## Other targets and publication

`linux-x86_64` is blocked after real-data tests found remaining pointer and graphics-layout problems. A successful link and rejection of a dummy map did not qualify it for play. Windows retains its earlier cross-build records; those records have not received the Linux preparation replacement or current gameplay qualification. Use the tested Linux target for this source profile.

The currently published `v0.1.6` selector remains immutable. This source continuation must receive a new signed package/version and matching catalog URL when released; editing the source tree does not update that existing download. No release is published by these documentation changes. See the host's [engine guide](https://github.com/centrexity/GamePKG/blob/main/engines/halo/README.md) for the reviewed implementation and test limits.
