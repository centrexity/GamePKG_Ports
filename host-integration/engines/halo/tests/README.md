These maintainer tests compile the actual patched native reader and filesystem helpers. They generate synthetic disc images; no game data is needed or extracted. Python is used only by this test runner.

The native Windows movie checks use the patched source, configured minimal
FFmpeg i686 MSVC build, and SDL3's x86 import library and DLL. Run from an
environment configured for that Windows SDK/CRT:

```sh
python3 tools/check_halo_windows_movies.py \
  --source /path/to/patched-opence \
  --ffmpeg-source /path/to/ffmpeg-9.0.2 \
  --ffmpeg-build /path/to/windows-ffmpeg-build \
  --sdl-include /path/to/SDL3/include \
  --sdl-lib /path/to/SDL3/lib/x86/SDL3.lib \
  --sdl-dll /path/to/SDL3.dll \
  --cc clang --output /tmp/new-windows-movie-checks
```

This only cross-builds and packages checks. Execute the positive executable
against all 25 generated image/scenario pairs in `cases.json` on Windows;
each must exit zero and report PASS. Both negative executables run against
the first valid sample and must exit 3: one omits the neutral SIMD reset,
the other omits restoration of the game's x87 precision. Error reporting goes
to stderr, with OS crash dialogs disabled for these test processes only.
The suite checks exact pixels, guards/stride, audio timing and gain, looping,
bad paths, malformed media, skip/reopen, neutral decoder controls and restoration
of rounding, exception flags, precision and SIMD state. It uses dummy audio and
creates no window. Synthetic checks do not establish audible or GPU playback,
full game accuracy or frame-time performance.

The shared-input integration's game-port policy is checked without media:

```sh
python3 engines/halo/tests/check_gamepad_ports.py --output /tmp/new-halo-ports64 --sanitize
python3 engines/halo/tests/check_gamepad_ports.py --output /tmp/new-halo-ports32 --cflag=-m32
```

It checks held-button deferral when entering split-screen, stable assignments
when another controller joins or leaves, fresh 64-bit generation handles,
sparse canonical slots and 4,096 topology changes. A deliberate compaction
regression must fail the exact existing-player assertion. The SDL3 service
fixture also exercises this policy using actual virtual controllers and shared
Rust snapshots. Native x86/x64 policy executables passed on both Windows test
machines; this does not qualify Windows Halo gameplay or physical controllers.
The Linux i686 prepared player plan includes this policy and shared-input
wiring; its end-to-end build is under qualification. Windows preparation and
complete split-screen gameplay remain unverified.

The compiler identity and preparation fixtures run with
`cargo test --lib external_engine::prepared`. The native link provenance check
additionally needs the engine's LLVM tools on PATH:

```sh
cargo test --lib external_engine::prepared::tests::optional_runtime_imports_cannot_hide_game_weak_symbols -- --ignored
```

It compiles actual objects and an archive: an optional runtime probe passes
only with its scoped exception, while the identical weak reference in game
code and an unrelated missing game function both fail.

After applying the reviewed engine patches to the pinned source:

```sh
python3 engines/halo/tests/check_disc_view.py --source /path/to/engine
python3 engines/halo/tests/check_disc_view.py --source /path/to/engine --i686-sysroot /path/to/engine/build/linux/third_party/sysroot
```

The 64-bit run enables AddressSanitizer and UndefinedBehaviorSanitizer. The 32-bit run exercises the original native game ABI. Cases cover traversal, invalid extents, cyclic links, duplicate names, missing required data, damaged descriptors, truncation, sparse reads beyond 2 GiB, EOF, read-only descriptors, stale directory handles, and concurrent readers. Failures stop the run.

On Windows, run the first command from an x86 Visual Studio developer prompt
with Clang and LLD on PATH. It compiles the same reader with the engine's Win32
filesystem and thread helpers. The large synthetic image uses an NTFS sparse
file and is removed with the temporary test directory. Small input images are
hashed before and after each case. These checks establish reader behavior;
they do not establish Windows gameplay, rendering or controller support.

Linux maintainers with Bubblewrap and user namespaces can check save persistence
separately. This compiles the actual map-cache path and path-translation
functions, then uses the catalog's exact RAM mount across two processes:

```sh
python3 engines/halo/tests/check_save_partition.py --source /path/to/engine
python3 engines/halo/tests/check_save_partition.py --source /path/to/engine --i686-sysroot /path/to/engine/build/linux/third_party/sysroot
```

Synthetic profiles, checkpoints and last-mission files must survive the restart;
synthetic map-cache bytes must disappear. A deliberate return to the original
cache drive must fail the empty-cache assertion. No existing player files are
used. The default run enables address/undefined-behavior checks; the second
uses the game's native 32-bit ABI. This does not qualify campaign progression.

The process-memory cache has no asset or graphics dependencies:

```sh
python3 engines/halo/tests/check_ram_cache.py --output /tmp/new-ram64 --sanitize
python3 engines/halo/tests/check_ram_cache.py --output /tmp/new-ram32 --cflag=-m32
```

It checks sparse zero-filled reads, cross-page writes, independent file positions,
EOF/overflow, truncation and re-extension, access modes, timestamps, logical
budgets, allocation-failure rollback and eight concurrent writers. Three broken
variants must fail their specific assertions. `--source` additionally verifies
the engine's copied header. On native Windows, supply that patched source and
an x86 developer environment; the test uses the engine's real Windows thread
helpers. The deliberate failure variants run only on Linux.

After a normal maintainer Windows cross-build has configured and built the
engine, this builds a separate fixture with its actual file, handle, thread,
disc and host-filesystem code:

```sh
python3 engines/halo/tests/build_windows_ram_facade.py --source /path/to/engine --output /tmp/new-windows-ram-facade
```

Execute the resulting EXE on Windows separately. It uses only synthetic in-memory
data and checks original handles, sequential/positioned reads and writes,
asynchronous completion during alertable waits, truncation, dates, attributes,
permissions, malformed cache paths and copy/move rejection. A successful build
receipt explicitly records that execution has not occurred. Neither fixture
establishes Windows gameplay or physical-controller support.

The HUD fixture recovers the pinned original scalar kernel and the adapted kernel from the reviewed patch. It needs no checkout, ROM, graphics device or game artwork:

```sh
python3 engines/halo/tests/check_hud_upscale.py --sanitize
python3 engines/halo/tests/check_hud_upscale.py --scalar
python3 engines/halo/tests/check_hud_upscale.py --cc clang --cflag=--target=i686-linux-gnu --cflag=-m32 --cflag=-msse2 --cflag=-mfpmath=sse --cflag=--sysroot=/path/to/engine/build/linux/third_party/sysroot
```

It compares every output byte and floating-point exception flag in 263,256 image cases across all four rounding modes, including exhaustive binary tap patterns, one-pixel edges, maximum dimensions and unaligned inputs. The default Linux Rust scaffold test runs both SIMD and scalar cases. `--source /path/to/engine` additionally verifies that the actual patched checkout contains the exact tested kernel. `--quick` and `--runner` support smaller cross-architecture fixture runs. Reported kernel timings are diagnostic; correctness does not depend on a machine-specific speed threshold.

`python3 engines/halo/tests/check_video_failure.py` checks normal, explicit headless, and standalone window-failure policies without game media.

The `Native Halo source safeguards` CI job fetches the pinned public source and applies the reviewed patches, then runs these malformed-image, sparse-offset, concurrent-read, exact-HUD and window-policy fixtures on actual native 64-bit and 32-bit code. It uses no game media. Python is a maintainer/CI tool, not a player requirement.

Movie fixtures generate an original three-frame black/white Bink clip and silent RDFT/DCT audio. They check every output pixel and padding byte, stride/offset bounds, frame timing, fractional audio samples across loops, mute/gain, cleanup/reopen, all four floating-point rounding modes, x87 precision and SSE state, bounded paths, malformed headers and truncated packets. A deliberately wrong i686 structure alignment must fail compilation. The 64-bit harness instruments both FFmpeg and the bridge with AddressSanitizer and UndefinedBehaviorSanitizer; the real game remains native i686.

`check_movie_view.py --source /path/to/patched/engine` separately checks maps/movie namespace separation, read-only metadata, traversal/ADS/NUL names, duplicate and cyclic directories, invalid extents, end-of-file and huge offsets, plus 8,000 concurrent reads. Its generated-disc cases run on native 32-bit and 64-bit sanitizer builds.

The following CI setup downloads only pinned open-source dependencies and builds dummy-audio SDL libraries for buffer tests. It opens no game window and needs no ROM:

```sh
python3 tools/check_opence_movie_codecs.py --source /path/to/patched/engine --work /tmp/new-movie32 --profile native32
python3 tools/check_opence_movie_codecs.py --source /path/to/patched/engine --work /tmp/new-movie64 --profile sanitized64
```

An existing reviewed player build can also supply its actual SDL library and prepared FFmpeg archives to `check_movie_playback.py`; see `--help`. These are decoder/buffer checks. GPU rendering, controller behavior and multiplayer require separate runtime checks.

The scripted-suspension fixture compiles the actual snapshot writer/reader, view application, transition handler, and original engine setter from the patched source. It checks 7,680 flag/shape/ownership/view round-trips and 291,816 truncated allocations per profile, including suspended units with camouflage, riding, damage and powerup fields. Thirteen application cases cover remote angular correction, active owner isolation, scripted owner cameras, angular velocities, non-cooperative isolation and rejection of invalid directions before partial writes. Parent/vehicle conversion is covered separately by the original-engine basis fixture. Repeated snapshots do not repeatedly clear velocity or airborne state, and unrelated flags survive. Three negative controls must fail the specific expected assertion: removing owner filtering, removing the non-cooperative guard, and disabling remote view application.

```sh
python3 engines/halo/tests/check_scripted_suspension.py --source /path/to/patched/engine --output /tmp/suspension32 --profile native32
python3 engines/halo/tests/check_scripted_suspension.py --source /path/to/patched/engine --output /tmp/suspension64 --profile sanitized64
```

The 64-bit fixture uses AddressSanitizer and UndefinedBehaviorSanitizer; it does not establish 64-bit gameplay support. Both fixtures use generated protocol values without game media. Network version 34 distinguishes the extended scripted-view snapshots from older peers.


The co-op runtime fixture extracts the patched prediction loop, simulation clock,
and vehicle-facing conversion. Each profile runs 26 clock cases (including the
17 earlier regressions), 14 scripted handoff cases, and 11,900 aiming-basis
round-trips using the original engine conversion and matrix functions. Six
negative controls per profile must fail their assertions: removing catch-up,
its rate guard, the ahead budget, fractional-frame preservation, scripted
position authority, or using the wrong parent-node camera basis.

```sh
python3 engines/halo/tests/check_coop_runtime.py --source /path/to/patched/engine --output /tmp/coop32 --profile native32
python3 engines/halo/tests/check_coop_runtime.py --source /path/to/patched/engine --output /tmp/coop64 --profile sanitized64
```

World, tag and device access are mocked in these asset-free fixtures. They prove
specific function behavior, including stale clock/invalid RTT handling, time
wrap, new game epochs, zero-tick render frames and original seat-basis rules.
Actual GPU playback, controller delivery, campaign progress, stalls, network
latency and frame times remain separate runtime checks. Python runs only in
maintainer tests; the player build and runtime do not use these scripts.

## Hardware graphics profiles

Run the actual patched GL loader with the asset-free fake-driver fixture:

```sh
python3 engines/halo/tests/check_gl_profile.py --source /path/to/patched/opence --sdl-include /path/to/SDL3/include --output /tmp/halo-gl64 --sanitize
python3 engines/halo/tests/check_gl_profile.py --source /path/to/patched/opence --sdl-include /path/to/SDL3/include --output /tmp/halo-gl32 --cflag=-m32
```

These are maintainer commands; players do not need Python. The fixture uses
the real loader and adapters, covering automatic/default/forced profiles,
36,864 typed query objects, 60 batch shapes, exact offsets/results, restored
bindings, extension refusals and zero synchronous driver-state reads during
query copies. Deliberately broken offset, binding and driver-read variants
must fail their corresponding assertions. The 64-bit run enables address,
undefined-behavior and leak checks; the second exercises the native 32-bit ABI.

This fixture does not validate GPU pixels or GLSL compilation. Those require
native hardware checks, including all built-in shader programs and real-disc
scene/HUD rendering. A clean exit or advancing simulation is insufficient:
a rejected shader can leave a clear-color frame while the simulation runs.
