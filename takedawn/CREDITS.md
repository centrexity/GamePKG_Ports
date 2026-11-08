This package selects the pinned Takedawn native implementation and the prepared
Rust game functions documented in GamePKG's engines/takedawn directory. The
source kit includes the original Takedawn, Xboxrecomp and C2Rust notices.
GamePKG's host integration is Apache-2.0 licensed.

Original game media is required from the user's supported Xbox USA ISO and is
not included. The ISO is read in place. Python and C2Rust are maintainer tools;
the player's native build uses the prepared Rust source and original C/C++ host
runtime. The package has no external Rust game dependencies.

The thumbnail is an original geometric racing illustration by GamePKG
contributors, under Apache-2.0. Its editable SVG is kept with the port artwork.
It contains no original game artwork or logo.

The 0.1.7 candidate uses GamePKG's reviewed v18 source kit. Its shared-results
input guard keeps menu buttons and prompts correct while the original game
replays the crash behind its results. Prepared Rust game functions are unchanged.
This is a host input adaptation; it replaces no original game function or media.

The kernel thread adapter resolves game thread objects to owned host thread
handles before querying or setting priorities. It retains the original game
functions, native graphics path, AppContainer isolation and memory limits.

The GPU diagnostic report ranks a snapshot rather than sorting the indexed
counter table. This preserves constant-time method lookup and correct counts
across reports; it changes no drawing or game function.

Kenney Input Prompts 1.5A — Kenney (https://kenney.nl/assets/input-prompts), CC0-1.0. Selected original vectors, their license and provenance hashes are included in the reviewed source kit under host/engines/takedawn/media-bridge/input-prompts. The host composites and renders them in advance for crisp 1×/2×/3×/4× prompts; players need no SVG renderer or Python. PlayStation/DualSense retain the existing Takedawn-owned glyph art because this Kenney download does not include PlayStation artwork.

Original Xbox button prompts are supplied by the player’s selected ISO, read in place and upscaled locally. Their pixels and enlarged caches are not included in this recipe. Xbox originals are the default; the Kenney artwork is retained for alternate controller/keyboard layouts.
