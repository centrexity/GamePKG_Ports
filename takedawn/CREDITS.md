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

The 0.1.5 candidate uses GamePKG's reviewed v16 source kit. Its shared-results
input guard keeps menu buttons and prompts correct while the original game
replays the crash behind its results. Prepared Rust game functions are unchanged.
This is a host input adaptation; it replaces no original game function or media.
