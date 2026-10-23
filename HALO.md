# Halo: Combat Evolved (OpenCE)

Selector for [OpenCE](https://github.com/OpenCommunityEdition/OpenCE) commit `ce95f7deed5bbf726e50a9d5fe5227437194cf3b`. It is not in the published catalog. Pack and install it from this checkout. GamePKG 0.1.17 or newer is required.

No Halo maps, disc images, or cache files are in this repository. OpenCE is AGPL-3.0-or-later and is not vendored. The host clones that commit, checks the license digest, and builds one reviewed target. Play runs that program in its own window. The package only declares data layouts. Fetch, compile, image checks, the desktop sandbox, and where game data is linked are generic host code, shared with any later engine of this kind.

## What you supply

A folder that contains either:

- `maps/ui.map` extracted from a Halo: Combat Evolved Xbox disc you own, or
- exactly one `.iso` or `.xiso` of that disc.

Those two shapes are the reviewed layouts in `manifest.toml`. Custom Edition caches stay in a `custom_maps` folder you keep beside those maps. Do not put any of those files in this repository.

## Targets

| Target | Command | Status |
| --- | --- | --- |
| `linux-i686` | `gamepkg build com.gamepkg.halo` on 64-bit or 32-bit Linux | Builds and runs. This is the default on x86_64 Linux, because the 64-bit target is blocked upstream. |
| `linux-x86_64` | `gamepkg build com.gamepkg.halo --engine-target linux-x86_64` | Refused before any compile. OpenCE's Linux prefix errors with `the Linux port targets 32-bit x86: game data structures assume 32-bit pointers`. |
| `windows-i686` | `gamepkg build com.gamepkg.halo --engine-target windows-i686` | Cross-compiles to a 32-bit PE. A Linux host will not launch it. |
| `windows-x64` | `gamepkg build com.gamepkg.halo --engine-target windows-x64` | Refused before any compile. OpenCE's Windows prefix errors with `the Windows port targets 32-bit x86: game data structures assume 32-bit pointers`. |

`GAMEPKG_ENGINE_SOURCE` can point at a checkout of the pinned commit. The host still checks the commit and the license digest. A Windows cross build also needs `GAMEPKG_MSVC_TOOLS` (the versioned `VC/Tools/MSVC/<version>` directory, with `include` and `lib`) and `GAMEPKG_WINDOWS_SDK` (the SDK directory whose `Include` and `Lib` each contain a versioned folder). `xwin splat --use-winsysroot-style` produces that layout. A plain `xwin splat` does not.

The Linux build needs Python 3, Ninja, Clang, CMake, pkg-config, wayland-scanner, and 32-bit glibc headers (`gcc-multilib`, `libc6-dev-i386`). The first portable compile downloads a Debian 11 sysroot and SDL3. The Windows cross build needs those tools plus `llvm-rc` and `lld` (`-fuse-ld=lld`). It does not use `--portable`: Clang 18 ignores `-march=x86-64` on `i686-pc-windows-msvc`, SSE2 stays disabled, and OpenCE's mbedtls sources then fail with `unknown type name '__m128i'`. The default `-march=native` enables SSE2, so the PE is tuned to the machine that built it.

MinGW is not a target. `i686-w64-mingw32` and `x86_64-w64-mingw32` both hit the Windows prefix `#error`, because that header requires `_M_IX86`, which Clang defines for `i686-pc-windows-msvc` and not for MinGW. There is no 64-bit or x32 OpenCE target. The Android ILP32 guest is `arm64_32`, not 64-bit x86. Cache files and saved games embed 32-bit pointers, so a 64-bit ABI would not read Xbox maps even if the prefix check were removed.

## Play

```sh
gamepkg pack halo -o /tmp/halo.gamepkg --key /path/to/creator.key
gamepkg install /tmp/halo.gamepkg --trust-creator
gamepkg build com.gamepkg.halo
gamepkg run com.gamepkg.halo --game-data /path/to/your/halo-folder
```

On Windows, build with `--engine-target windows-i686` and run that PE on Windows. The Linux host caches a Windows build and then refuses to start it.

Or use Build and Play in the desktop library, then choose the folder. Set the memory ceiling to at least 2048 MiB. The engine opens its own window. Saves and a disc extraction stay under the library runtime directory. An extracted `maps/` folder is linked from a private content directory so the engine's log is not written into your folder. Automatic upstream updates are turned off.

`HALO_HIDDEN_WINDOW`, `HALO_NULL_RENDERER`, and `HALO_EXIT_AFTER` are passed through when set. Leave them unset for normal play.

Bubblewrap, when installed on Linux, starts the game without a network. 32-bit ALSA (`libasound.so.2` for i386) is needed for sound on the Linux i686 build; otherwise the game starts with SDL's dummy audio driver.

This is not the Cube 2 frame bridge. There is no Wasm build and no catalog download until a maintainer publishes a release. See `engines/halo/README.md` in the GamePKG host.
